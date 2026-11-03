use super::ScanContext;
use super::builds::tilde_path;
use super::caches::{make_item, run_stdout};
use crate::model::{Action, Category, Item, Risk};
use crate::size::disk_usage;
use rayon::prelude::*;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const SYSTEM_APPLICATIONS: &str = "/Applications";
const USER_APPLICATIONS: &str = "Applications";
const SYSTEM_PREFIX: &str = "/System";
const APP_EXTENSION: &str = "app";
const SAFARI: &str = "Safari.app";
const TERM_PROGRAM_VAR: &str = "TERM_PROGRAM";
const LAST_USED_ATTRIBUTE: &str = "kMDItemLastUsedDate";
const NEVER_OPENED: &str = "no recorded use (Spotlight or app data)";
const BUNDLE_ID_ATTRIBUTE: &str = "kMDItemCFBundleIdentifier";
const PREFERENCES_DIR: &str = "Library/Preferences";
const CONTAINERS_DIR: &str = "Library/Containers";
const APP_SUPPORT_DIR: &str = "Library/Application Support";
const PLIST_EXTENSION: &str = "plist";
const SECS_PER_DAY: i64 = 86_400;
const SECS_PER_HOUR: i64 = 3_600;
const SECS_PER_MINUTE: i64 = 60;
const MAX_MONTH: i64 = 12;
const MAX_DAY: i64 = 31;
const MAX_HOUR: i64 = 23;
const MAX_MINUTE_OR_SECOND: i64 = 59;
const YEARS_PER_ERA: i64 = 400;
const DAYS_PER_ERA: i64 = 146_097;
const EPOCH_SHIFT_DAYS: i64 = 719_468;

const TERMINAL_APPS: [(&str, &str); 7] = [
    ("iTerm.app", "iTerm.app"),
    ("Apple_Terminal", "Terminal.app"),
    ("ghostty", "Ghostty.app"),
    ("WezTerm", "WezTerm.app"),
    ("WarpTerminal", "Warp.app"),
    ("Hyper", "Hyper.app"),
    ("vscode", "Visual Studio Code.app"),
];

pub fn scan(ctx: &ScanContext) -> Vec<Item> {
    let Some(mdls) = ctx.tools.get("mdls") else {
        return Vec::new();
    };
    let terminal = std::env::var(TERM_PROGRAM_VAR)
        .ok()
        .and_then(|program| terminal_app_name(&program));
    let now = now_secs();
    app_bundles(ctx)
        .into_par_iter()
        .filter(|path| !is_protected(path, terminal.as_deref()))
        .filter_map(|path| app_item(ctx, mdls, path, now))
        .collect()
}

pub fn terminal_app_name(term_program: &str) -> Option<String> {
    TERMINAL_APPS
        .iter()
        .find(|(program, _)| *program == term_program)
        .map(|(_, app)| (*app).to_string())
}

pub fn parse_mdls_date(text: &str) -> Option<i64> {
    let mut parts = text.split_whitespace();
    let (year, month, day) = parse_date(parts.next()?)?;
    let clock = parse_clock(parts.next()?)?;
    let offset = parts.next().map_or(Some(0), parse_offset)?;
    Some(days_from_civil(year, month, day) * SECS_PER_DAY + clock - offset)
}

pub fn staleness(last_used: Option<i64>, now: i64, stale_days: u64) -> Option<String> {
    let Some(stamp) = last_used else {
        return Some(NEVER_OPENED.to_string());
    };
    let days = u64::try_from((now - stamp) / SECS_PER_DAY).unwrap_or(0);
    (days > stale_days).then(|| format!("last opened {days} days ago"))
}

fn parse_date(text: &str) -> Option<(i64, i64, i64)> {
    let mut fields = text.split('-').map(|f| f.parse::<i64>().ok());
    let (year, month, day) = (fields.next()??, fields.next()??, fields.next()??);
    let valid = (1..=MAX_MONTH).contains(&month) && (1..=MAX_DAY).contains(&day);
    valid.then_some((year, month, day))
}

fn parse_clock(text: &str) -> Option<i64> {
    let mut fields = text.split(':').map(|f| f.parse::<i64>().ok());
    let (hour, minute, second) = (fields.next()??, fields.next()??, fields.next()??);
    let valid = (0..=MAX_HOUR).contains(&hour)
        && (0..=MAX_MINUTE_OR_SECOND).contains(&minute)
        && (0..=MAX_MINUTE_OR_SECOND).contains(&second);
    valid.then_some(hour * SECS_PER_HOUR + minute * SECS_PER_MINUTE + second)
}

fn parse_offset(text: &str) -> Option<i64> {
    let sign = match text.chars().next()? {
        '+' => 1,
        '-' => -1,
        _ => return None,
    };
    let digits = text.get(1..)?;
    let (hours, minutes) = (
        digits.get(..2)?.parse::<i64>().ok()?,
        digits.get(2..4)?.parse::<i64>().ok()?,
    );
    Some(sign * (hours * SECS_PER_HOUR + minutes * SECS_PER_MINUTE))
}

// Howard Hinnant's days_from_civil: exact for the proleptic Gregorian calendar.
fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = year.div_euclid(YEARS_PER_ERA);
    let year_of_era = year - era * YEARS_PER_ERA;
    let shifted_month = (month + 9) % 12;
    let day_of_year = (153 * shifted_month + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * DAYS_PER_ERA + day_of_era - EPOCH_SHIFT_DAYS
}

fn now_secs() -> i64 {
    let elapsed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    i64::try_from(elapsed.as_secs()).unwrap_or(0)
}

fn app_bundles(ctx: &ScanContext) -> Vec<PathBuf> {
    let roots = [
        PathBuf::from(SYSTEM_APPLICATIONS),
        ctx.home.join(USER_APPLICATIONS),
    ];
    roots.iter().flat_map(|root| list_apps(root)).collect()
}

fn list_apps(root: &Path) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(root) else {
        return Vec::new();
    };
    entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == APP_EXTENSION))
        .collect()
}

fn is_protected(path: &Path, terminal: Option<&str>) -> bool {
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned());
    if name.as_deref() == Some(SAFARI) || name.as_deref() == terminal {
        return true;
    }
    fs::canonicalize(path).is_ok_and(|real| real.starts_with(SYSTEM_PREFIX))
}

fn mdls_value(mdls: &Path, attribute: &str, path: &Path) -> Option<String> {
    let raw = run_stdout(mdls, &["-raw", "-name", attribute, path.to_str()?])?;
    let value = raw.trim();
    (!value.is_empty() && value != "(null)").then(|| value.to_string())
}

pub fn app_data_paths(home: &Path, bundle_id: Option<&str>, name: &str) -> Vec<PathBuf> {
    let mut paths = vec![home.join(APP_SUPPORT_DIR).join(name)];
    if let Some(id) = bundle_id {
        paths.push(
            home.join(PREFERENCES_DIR)
                .join(format!("{id}.{PLIST_EXTENSION}")),
        );
        paths.push(home.join(CONTAINERS_DIR).join(id));
        paths.push(home.join(APP_SUPPORT_DIR).join(id));
    }
    paths
}

pub fn latest_activity(paths: &[PathBuf]) -> Option<i64> {
    paths
        .iter()
        .filter_map(|path| fs::metadata(path).and_then(|meta| meta.modified()).ok())
        .filter_map(|time| time.duration_since(UNIX_EPOCH).ok())
        .filter_map(|elapsed| i64::try_from(elapsed.as_secs()).ok())
        .max()
}

fn last_used(ctx: &ScanContext, mdls: &Path, path: &Path) -> Option<i64> {
    let spotlight =
        mdls_value(mdls, LAST_USED_ATTRIBUTE, path).and_then(|raw| parse_mdls_date(&raw));
    let bundle_id = mdls_value(mdls, BUNDLE_ID_ATTRIBUTE, path);
    let name = path.file_stem()?.to_string_lossy().into_owned();
    let data = latest_activity(&app_data_paths(&ctx.home, bundle_id.as_deref(), &name));
    spotlight.max(data)
}

fn app_item(ctx: &ScanContext, mdls: &Path, path: PathBuf, now: i64) -> Option<Item> {
    let status = staleness(last_used(ctx, mdls, &path), now, ctx.config.stale_app_days)?;
    let bytes = disk_usage(std::slice::from_ref(&path));
    let label = path.file_stem()?.to_string_lossy().into_owned();
    let detail = format!("{status} · {}", tilde_path(&path, &ctx.home));
    Some(make_item(
        Category::UnusedApps,
        label,
        detail,
        bytes,
        Risk::Review,
        Action::MoveToTrash(path),
    ))
}
