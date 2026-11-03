use super::ScanContext;
use super::support::located_detail;
use crate::model::{Action, Category, Item, Risk};
use crate::process::{DEFAULT_TIMEOUT, run_stdout, stdout_of};
use crate::size::path_usage;
use crate::time::signed::{SECS_PER_DAY, SECS_PER_HOUR, SECS_PER_MINUTE};
use crate::time::{now_secs, unix_days_from_date};
use crate::tools;
use rayon::prelude::*;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::UNIX_EPOCH;

const SYSTEM_APPLICATIONS: &str = "/Applications";
const USER_APPLICATIONS: &str = "Applications";
const SYSTEM_PREFIX: &str = "/System";
const APP_EXTENSION: &str = "app";
const SAFARI: &str = "Safari.app";
const TERM_PROGRAM_VAR: &str = "TERM_PROGRAM";
const BUNDLE_ID_ATTRIBUTE: &str = "kMDItemCFBundleIdentifier";
const LAST_USED_ATTRIBUTE: &str = "kMDItemLastUsedDate";
const MDLS_RAW_FLAG: &str = "-raw";
const MDLS_NAME_FLAG: &str = "-name";
const MDLS_NULL: &str = "(null)";
const MDLS_FIELD_SEPARATOR: char = '\0';
const MDLS_FIELDS_PER_APP: usize = 2;
const NO_USE_RECORDED_NOTE: &str = "no recorded use (Spotlight or app data)";
const DATE_SEPARATOR: char = '-';
const CLOCK_SEPARATOR: char = ':';
const PREFERENCES_DIR: &str = "Library/Preferences";
const CONTAINERS_DIR: &str = "Library/Containers";
const APP_SUPPORT_DIR: &str = "Library/Application Support";
const PLIST_EXTENSION: &str = "plist";
const MAX_MONTH: i64 = 12;
const MAX_DAY: i64 = 31;
const MAX_HOUR: i64 = 23;
const MAX_MINUTE_OR_SECOND: i64 = 59;

const TERMINAL_APPS: [(&str, &str); 7] = [
    ("iTerm.app", "iTerm.app"),
    ("Apple_Terminal", "Terminal.app"),
    ("ghostty", "Ghostty.app"),
    ("WezTerm", "WezTerm.app"),
    ("WarpTerminal", "Warp.app"),
    ("Hyper", "Hyper.app"),
    ("vscode", "Visual Studio Code.app"),
];

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Spotlight {
    pub bundle_id: Option<String>,
    pub last_used: Option<i64>,
}

pub fn scan(ctx: &ScanContext) -> Vec<Item> {
    let Some(mdls) = ctx.tools.get(tools::MDLS) else {
        return Vec::new();
    };
    let terminal = std::env::var(TERM_PROGRAM_VAR)
        .ok()
        .and_then(|program| terminal_app_name(&program));
    let now = i64::try_from(now_secs()).unwrap_or(0);
    let apps: Vec<PathBuf> = app_bundles(ctx)
        .into_par_iter()
        .filter(|path| !is_protected(path, terminal.as_deref()))
        .collect();
    let records = spotlight_records(mdls, &apps);
    apps.into_par_iter()
        .zip(records)
        .filter_map(|(path, spotlight)| app_item(ctx, path, &spotlight, now))
        .collect()
}

pub fn terminal_app_name(term_program: &str) -> Option<String> {
    TERMINAL_APPS
        .iter()
        .find(|(program, _)| *program == term_program)
        .map(|(_, app)| (*app).to_owned())
}

pub fn parse_mdls_date(text: &str) -> Option<i64> {
    let mut parts = text.split_whitespace();
    let [year, month, day] = parse_date(parts.next()?)?;
    let clock = parse_clock(parts.next()?)?;
    let offset = parts.next().map_or(Some(0), parse_offset)?;
    Some(unix_days_from_date(year, month, day) * SECS_PER_DAY + clock - offset)
}

pub fn parse_mdls_batch(raw: &str, app_count: usize) -> Option<Vec<Spotlight>> {
    let fields: Vec<&str> = raw.split(MDLS_FIELD_SEPARATOR).collect();
    if fields.len() != app_count * MDLS_FIELDS_PER_APP {
        return None;
    }
    let (pairs, _) = fields.as_chunks::<MDLS_FIELDS_PER_APP>();
    Some(pairs.iter().map(spotlight_of).collect())
}

pub fn stale_reason(latest_use: Option<i64>, now: i64, stale_days: u64) -> Option<String> {
    let Some(stamp) = latest_use else {
        return Some(NO_USE_RECORDED_NOTE.to_owned());
    };
    let days = u64::try_from((now - stamp) / SECS_PER_DAY).unwrap_or(0);
    (days > stale_days).then(|| format!("last opened {days} days ago"))
}

fn spotlight_of(pair: &[&str; MDLS_FIELDS_PER_APP]) -> Spotlight {
    let [bundle_id, last_used] = pair;
    Spotlight {
        bundle_id: mdls_text(bundle_id),
        last_used: mdls_text(last_used).and_then(|raw| parse_mdls_date(&raw)),
    }
}

fn mdls_text(raw: &str) -> Option<String> {
    let value = raw.trim();
    (!value.is_empty() && value != MDLS_NULL).then(|| value.to_owned())
}

fn parse_triple(text: &str, separator: char) -> Option<[i64; 3]> {
    let mut fields = text.split(separator).map(|field| field.parse::<i64>().ok());
    Some([fields.next()??, fields.next()??, fields.next()??])
}

fn parse_date(text: &str) -> Option<[i64; 3]> {
    let date @ [_, month, day] = parse_triple(text, DATE_SEPARATOR)?;
    let valid = (1..=MAX_MONTH).contains(&month) && (1..=MAX_DAY).contains(&day);
    valid.then_some(date)
}

fn parse_clock(text: &str) -> Option<i64> {
    let [hour, minute, second] = parse_triple(text, CLOCK_SEPARATOR)?;
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
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned());
    if name.as_deref() == Some(SAFARI) || name.as_deref() == terminal {
        return true;
    }
    fs::canonicalize(path).is_ok_and(|real| real.starts_with(SYSTEM_PREFIX))
}

fn spotlight_records(mdls: &Path, apps: &[PathBuf]) -> Vec<Spotlight> {
    batched_spotlight(mdls, apps).unwrap_or_else(|| {
        apps.par_iter()
            .map(|app| single_spotlight(mdls, app))
            .collect()
    })
}

fn batched_spotlight(mdls: &Path, apps: &[PathBuf]) -> Option<Vec<Spotlight>> {
    if apps.is_empty() {
        return Some(Vec::new());
    }
    let paths = apps
        .iter()
        .map(|app| app.to_str())
        .collect::<Option<Vec<&str>>>()?;
    let mut command = Command::new(mdls);
    command
        .args([MDLS_RAW_FLAG, MDLS_NAME_FLAG, BUNDLE_ID_ATTRIBUTE])
        .args([MDLS_NAME_FLAG, LAST_USED_ATTRIBUTE])
        .args(paths);
    parse_mdls_batch(&stdout_of(&mut command, DEFAULT_TIMEOUT)?, apps.len())
}

fn single_spotlight(mdls: &Path, app: &Path) -> Spotlight {
    Spotlight {
        bundle_id: mdls_value(mdls, BUNDLE_ID_ATTRIBUTE, app),
        last_used: mdls_value(mdls, LAST_USED_ATTRIBUTE, app).and_then(|raw| parse_mdls_date(&raw)),
    }
}

fn mdls_value(mdls: &Path, attribute: &str, path: &Path) -> Option<String> {
    let args = [MDLS_RAW_FLAG, MDLS_NAME_FLAG, attribute, path.to_str()?];
    mdls_text(&run_stdout(mdls, &args, DEFAULT_TIMEOUT)?)
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

fn latest_use_epoch_secs(ctx: &ScanContext, path: &Path, spotlight: &Spotlight) -> Option<i64> {
    let name = file_stem_label(path)?;
    let data_paths = app_data_paths(&ctx.home, spotlight.bundle_id.as_deref(), &name);
    spotlight.last_used.max(latest_activity(&data_paths))
}

fn app_item(ctx: &ScanContext, path: PathBuf, spotlight: &Spotlight, now: i64) -> Option<Item> {
    let latest_use = latest_use_epoch_secs(ctx, &path, spotlight);
    let status = stale_reason(latest_use, now, ctx.config.stale_app_days)?;
    let bytes = path_usage(&path);
    let label = file_stem_label(&path)?;
    let detail = located_detail(&status, &path, &ctx.home);
    Some(Item::new(
        Category::UnusedApps,
        label,
        detail,
        bytes,
        Risk::Review,
        Action::MoveToTrash(path),
    ))
}

fn file_stem_label(path: &Path) -> Option<String> {
    Some(path.file_stem()?.to_string_lossy().into_owned())
}
