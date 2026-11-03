use super::owners::{OWNER_RUNNING_NOTE, named_owner_in};
use crate::model::{Action, Category, Item, Risk};
use crate::platform::app_caches_dir;
use crate::scan::ScanContext;
use crate::scan::support::{file_label, located_detail, measured};
use rayon::prelude::*;
use std::fs;
use std::path::PathBuf;

const MIN_APP_CACHE_BYTES: u64 = 50_000_000;
const APPLE_CACHE_PREFIX: &str = "com.apple.";
const UPDATER_SUFFIX: &str = "-updater";
const REGENERABLE_NOTE: &str = "Regenerable";
const UNKNOWN_NOTE: &str = "Unknown app cache, review before deleting";

const EXCLUDED_NAMES: [&str; 10] = [
    "CloudKit",
    "GeoServices",
    "pip",
    "Yarn",
    "yarn",
    "go-build",
    "Homebrew",
    "pnpm",
    "uv",
    "huggingface",
];

const SAFE_NAMES: [&str; 11] = [
    "ms-playwright",
    "electron",
    "electron-builder",
    "node-gyp",
    "Google",
    "Firefox",
    "Steam",
    "google-chrome",
    "chromium",
    "mozilla",
    "thumbnails",
];

struct Candidate {
    path: PathBuf,
    risk: Risk,
}

pub fn app_cache_risk(name: &str) -> Option<Risk> {
    if name.starts_with(APPLE_CACHE_PREFIX) || EXCLUDED_NAMES.contains(&name) {
        return None;
    }
    let regenerable = SAFE_NAMES.contains(&name) || name.ends_with(UPDATER_SUFFIX);
    Some(if regenerable {
        Risk::Safe
    } else {
        Risk::Review
    })
}

pub fn app_cache_risk_now(name: &str, processes: &str) -> Option<Risk> {
    match app_cache_risk(name)? {
        Risk::Safe if named_owner_in(name, processes) => Some(Risk::Review),
        risk => Some(risk),
    }
}

pub(super) fn app_caches(ctx: &ScanContext, processes: &str) -> Vec<Item> {
    let Ok(entries) = fs::read_dir(app_caches_dir(&ctx.home)) else {
        return Vec::new();
    };
    let candidates: Vec<Candidate> = entries
        .flatten()
        .filter_map(|entry| candidate(&entry, processes))
        .collect();
    candidates
        .into_par_iter()
        .filter_map(|candidate| app_cache_item(ctx, candidate))
        .collect()
}

fn candidate(entry: &fs::DirEntry, processes: &str) -> Option<Candidate> {
    if !entry.file_type().ok()?.is_dir() {
        return None;
    }
    let name = entry.file_name().to_string_lossy().into_owned();
    Some(Candidate {
        path: entry.path(),
        risk: app_cache_risk_now(&name, processes)?,
    })
}

fn app_cache_note(risk: Risk, label: &str) -> &'static str {
    match risk {
        Risk::Safe => REGENERABLE_NOTE,
        _ if app_cache_risk(label) == Some(Risk::Safe) => OWNER_RUNNING_NOTE,
        _ => UNKNOWN_NOTE,
    }
}

fn app_cache_item(ctx: &ScanContext, candidate: Candidate) -> Option<Item> {
    let Candidate { path, risk } = candidate;
    let bytes = measured(&path).filter(|bytes| *bytes >= MIN_APP_CACHE_BYTES)?;
    let label = file_label(&path)?;
    let detail = located_detail(app_cache_note(risk, &label), &path, &ctx.home);
    Some(Item::new(
        Category::AppCaches,
        label,
        detail,
        bytes,
        risk,
        Action::DeletePath(path),
    ))
}
