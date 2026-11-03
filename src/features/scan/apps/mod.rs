mod mdls;

pub use mdls::{Spotlight, parse_mdls_batch};

use super::ScanContext;
use super::support::located_detail;
use crate::domain::model::{Action, Category, Item, Risk};
use crate::infra::fs::size::path_usage;
use crate::infra::sys::tools;
use crate::infra::util::time::now_secs;
use crate::infra::util::time::signed::SECS_PER_DAY;
use mdls::spotlight_records;
use rayon::prelude::*;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

const SYSTEM_APPLICATIONS: &str = "/Applications";
const USER_APPLICATIONS: &str = "Applications";
const SYSTEM_PREFIX: &str = "/System";
const APP_EXTENSION: &str = "app";
const SAFARI: &str = "Safari.app";
const TERM_PROGRAM_VAR: &str = "TERM_PROGRAM";
const NO_USE_RECORDED_NOTE: &str = "no recorded use (Spotlight or app data)";
const PREFERENCES_DIR: &str = "Library/Preferences";
const CONTAINERS_DIR: &str = "Library/Containers";
const APP_SUPPORT_DIR: &str = "Library/Application Support";
const PLIST_EXTENSION: &str = "plist";

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
    let Some(mdls) = ctx.tools.get(tools::MDLS) else {
        return Vec::new();
    };
    let terminal = std::env::var(TERM_PROGRAM_VAR)
        .ok()
        .and_then(|program| terminal_app_name(&program));
    let now = i64::try_from(now_secs()).unwrap_or(0);
    let apps: Vec<PathBuf> = app_bundles(ctx)
        .into_par_iter()
        .filter(|path| !is_protected(path, terminal))
        .collect();
    let records = spotlight_records(mdls, &apps);
    apps.into_par_iter()
        .zip(records)
        .filter_map(|(path, spotlight)| app_item(ctx, path, &spotlight, now))
        .collect()
}

pub fn terminal_app_name(term_program: &str) -> Option<&'static str> {
    TERMINAL_APPS
        .iter()
        .find(|(program, _)| *program == term_program)
        .map(|(_, app)| *app)
}

pub fn stale_reason(latest_use: Option<i64>, now: i64, stale_days: u64) -> Option<String> {
    let Some(stamp) = latest_use else {
        return Some(NO_USE_RECORDED_NOTE.to_owned());
    };
    let days = u64::try_from((now - stamp) / SECS_PER_DAY).unwrap_or(0);
    (days > stale_days).then(|| format!("last opened {days} days ago"))
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
    let name = path.file_name();
    if name == Some(OsStr::new(SAFARI)) || name == terminal.map(OsStr::new) {
        return true;
    }
    fs::canonicalize(path).is_ok_and(|real| real.starts_with(SYSTEM_PREFIX))
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
