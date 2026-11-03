use super::ScanContext;
use super::caches::{make_item, measured};
use crate::model::{Action, Category, Item, Risk};
use rayon::prelude::*;
use std::fs;
use std::path::PathBuf;
use std::time::SystemTime;

const DOWNLOADS_DIR: &str = "Downloads";
const SECS_PER_DAY: u64 = 86_400;
const INSTALLER_EXTENSIONS: [&str; 3] = ["dmg", "pkg", "iso"];
const INSTALLER_GRACE_DAYS: u64 = 7;

pub fn scan(ctx: &ScanContext) -> Vec<Item> {
    let Ok(entries) = fs::read_dir(ctx.home.join(DOWNLOADS_DIR)) else {
        return Vec::new();
    };
    let stale_days = ctx.config.stale_download_days;
    let candidates: Vec<(PathBuf, Risk, u64)> = entries
        .flatten()
        .filter_map(|entry| candidate(&entry, stale_days))
        .collect();
    candidates
        .into_par_iter()
        .filter_map(|(path, risk, age)| download_item(path, risk, age))
        .collect()
}

pub fn is_installer(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    INSTALLER_EXTENSIONS.iter().any(|ext| {
        lower
            .rsplit_once('.')
            .is_some_and(|(_, found)| found == *ext)
    })
}

pub fn classify_download(name: &str, age_days: u64, stale_days: u64) -> Option<Risk> {
    if is_installer(name) && age_days > INSTALLER_GRACE_DAYS {
        return Some(Risk::Safe);
    }
    if is_installer(name) {
        return Some(Risk::Review);
    }
    (age_days > stale_days).then_some(Risk::Review)
}

fn candidate(entry: &fs::DirEntry, stale_days: u64) -> Option<(PathBuf, Risk, u64)> {
    let name = entry.file_name().to_string_lossy().into_owned();
    if name.starts_with('.') {
        return None;
    }
    let modified = entry.metadata().ok()?.modified().ok()?;
    let age_days = SystemTime::now()
        .duration_since(modified)
        .unwrap_or_default()
        .as_secs()
        / SECS_PER_DAY;
    Some((
        entry.path(),
        classify_download(&name, age_days, stale_days)?,
        age_days,
    ))
}

fn download_item(path: PathBuf, risk: Risk, age_days: u64) -> Option<Item> {
    let bytes = measured(&path)?;
    let label = path.file_name()?.to_string_lossy().into_owned();
    let kind = match (is_installer(&label), risk) {
        (true, Risk::Safe) => "Installer",
        (true, _) => "Recent installer, maybe still needed",
        _ => "Old download",
    };
    let detail = format!("{kind} · modified {age_days} days ago");
    Some(make_item(
        Category::Downloads,
        label,
        detail,
        bytes,
        risk,
        Action::MoveToTrash(path),
    ))
}
