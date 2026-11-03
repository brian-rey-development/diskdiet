use super::ScanContext;
use super::support::{file_label, measured};
use crate::domain::model::{Action, Category, Item, Risk};
use crate::infra::util::time::age_days;
use rayon::prelude::*;
use std::fs;
use std::path::PathBuf;

const DOWNLOADS_DIR: &str = "Downloads";
const INSTALLER_EXTENSIONS: [&str; 6] = ["dmg", "pkg", "iso", "deb", "rpm", "appimage"];
const INSTALLER_GRACE_DAYS: u64 = 7;
const HIDDEN_PREFIX: char = '.';

#[derive(Clone, Copy)]
enum DownloadKind {
    Installer,
    RecentInstaller,
    Old,
}

impl DownloadKind {
    fn risk(self) -> Risk {
        match self {
            Self::Installer => Risk::Safe,
            Self::RecentInstaller | Self::Old => Risk::Review,
        }
    }

    fn description(self) -> &'static str {
        match self {
            Self::Installer => "Installer",
            Self::RecentInstaller => "Recent installer, maybe still needed",
            Self::Old => "Old download",
        }
    }
}

struct Candidate {
    path: PathBuf,
    kind: DownloadKind,
    age_days: u64,
}

pub fn scan(ctx: &ScanContext) -> Vec<Item> {
    let Ok(entries) = fs::read_dir(ctx.home.join(DOWNLOADS_DIR)) else {
        return Vec::new();
    };
    let stale_days = ctx.config.stale_download_days;
    let candidates: Vec<Candidate> = entries
        .flatten()
        .filter_map(|entry| candidate(&entry, stale_days))
        .collect();
    candidates
        .into_par_iter()
        .filter_map(download_item)
        .collect()
}

pub fn is_installer(name: &str) -> bool {
    name.rsplit_once('.').is_some_and(|(_, extension)| {
        INSTALLER_EXTENSIONS
            .iter()
            .any(|known| extension.eq_ignore_ascii_case(known))
    })
}

pub fn classify_download(name: &str, age_days: u64, stale_days: u64) -> Option<Risk> {
    download_kind(name, age_days, stale_days).map(DownloadKind::risk)
}

fn download_kind(name: &str, age_days: u64, stale_days: u64) -> Option<DownloadKind> {
    match (is_installer(name), age_days > INSTALLER_GRACE_DAYS) {
        (true, true) => Some(DownloadKind::Installer),
        (true, false) => Some(DownloadKind::RecentInstaller),
        (false, _) => (age_days > stale_days).then_some(DownloadKind::Old),
    }
}

fn candidate(entry: &fs::DirEntry, stale_days: u64) -> Option<Candidate> {
    let name = entry.file_name().to_string_lossy().into_owned();
    if name.starts_with(HIDDEN_PREFIX) {
        return None;
    }
    let modified = entry.metadata().ok()?.modified().ok()?;
    let age_days = age_days(modified);
    Some(Candidate {
        path: entry.path(),
        kind: download_kind(&name, age_days, stale_days)?,
        age_days,
    })
}

fn download_item(candidate: Candidate) -> Option<Item> {
    let Candidate {
        path,
        kind,
        age_days,
    } = candidate;
    let bytes = measured(&path)?;
    let label = file_label(&path)?;
    let detail = format!("{} · modified {age_days} days ago", kind.description());
    Some(Item::new(
        Category::Downloads,
        label,
        detail,
        bytes,
        kind.risk(),
        Action::MoveToTrash(path),
    ))
}
