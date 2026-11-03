use super::ScanContext;
use crate::model::{Action, Category, Item, Risk};
use crate::size::{disk_usage, format_bytes};
use rayon::prelude::*;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

const SECS_PER_DAY: u64 = 86_400;
const PYVENV_CONFIG: &str = "pyvenv.cfg";
const DAYS_PER_MONTH: u64 = 30;
const DAYS_PER_YEAR: u64 = 365;
const MONTHS_THRESHOLD_DAYS: u64 = 60;
const MAX_WALK_DEPTH: usize = 12;
const GIT_DIR: &str = ".git";
const NODE_MODULES: &str = "node_modules";
const PACKAGE_JSON: &str = "package.json";
const LINKED_WORKTREE_MARKER: &str = "/worktrees/";
const VENV_HINT: &str = "run `uv sync` to recreate";

type Found = BTreeMap<PathBuf, (Category, Risk)>;

pub fn plural_s(count: u64) -> &'static str {
    if count == 1 { "" } else { "s" }
}

pub fn format_age(secs: u64) -> String {
    let days = secs / SECS_PER_DAY;
    match days {
        0 => "today".to_string(),
        1..MONTHS_THRESHOLD_DAYS => format!("{days} day{} ago", plural_s(days)),
        MONTHS_THRESHOLD_DAYS..DAYS_PER_YEAR => {
            let months = days / DAYS_PER_MONTH;
            format!("{months} months ago")
        }
        _ => {
            let years = days / DAYS_PER_YEAR;
            format!("{years} year{} ago", plural_s(years))
        }
    }
}

pub fn secs_since(time: SystemTime) -> u64 {
    SystemTime::now()
        .duration_since(time)
        .map_or(0, |elapsed| elapsed.as_secs())
}

pub fn tilde_path(path: &Path, home: &Path) -> String {
    match path.strip_prefix(home) {
        Ok(rest) if rest.as_os_str().is_empty() => "~".to_string(),
        Ok(rest) => format!("~/{}", rest.display()),
        Err(_) => path.display().to_string(),
    }
}

fn is_real_dir(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok_and(|meta| meta.is_dir())
}

fn has_any_file(dir: &Path, names: &[&str]) -> bool {
    names.iter().any(|name| dir.join(name).is_file())
}

pub fn classify(dir: &Path) -> Option<(Category, Risk)> {
    if !is_real_dir(dir) {
        return None;
    }
    let name = dir.file_name()?.to_str()?;
    let parent = dir.parent()?;
    let build = |manifests: &[&str], risk: Risk| {
        has_any_file(parent, manifests).then_some((Category::BuildArtifacts, risk))
    };
    match name {
        "target" => build(&["Cargo.toml"], Risk::Safe),
        ".next" => build(&[PACKAGE_JSON], Risk::Safe),
        ".turbo" => build(&[PACKAGE_JSON, "turbo.json"], Risk::Safe),
        NODE_MODULES => build(&[PACKAGE_JSON], Risk::Review),
        ".venv" if dir.join(PYVENV_CONFIG).is_file() => Some((Category::PythonEnvs, Risk::Review)),
        _ => None,
    }
}

fn is_linked_worktree(dir: &Path) -> bool {
    fs::read_to_string(dir.join(GIT_DIR))
        .is_ok_and(|text| text.starts_with("gitdir:") && text.contains(LINKED_WORKTREE_MARKER))
}

fn node_modules_is_stale(dir: &Path, stale_days: u64) -> bool {
    let modified = dir
        .parent()
        .map(|parent| parent.join(PACKAGE_JSON))
        .and_then(|manifest| fs::metadata(manifest).ok())
        .and_then(|meta| meta.modified().ok());
    modified.is_some_and(|time| secs_since(time) >= stale_days.saturating_mul(SECS_PER_DAY))
}

fn is_excluded(dir: &Path, ctx: &ScanContext) -> bool {
    ctx.config
        .exclude
        .iter()
        .any(|excluded| dir.starts_with(excluded))
}

fn child_dirs(dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    entries
        .flatten()
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_dir()))
        .map(|entry| entry.path())
        .collect()
}

fn walk(dir: &Path, depth: usize, ctx: &ScanContext, found: &mut Found) {
    if depth > MAX_WALK_DEPTH || is_excluded(dir, ctx) || is_linked_worktree(dir) {
        return;
    }
    for child in child_dirs(dir) {
        visit(&child, depth, ctx, found);
    }
}

fn visit(child: &Path, depth: usize, ctx: &ScanContext, found: &mut Found) {
    let name = child.file_name().and_then(|name| name.to_str());
    if name == Some(GIT_DIR) {
        return;
    }
    if let Some(matched) = classify(child) {
        if should_offer(child, name, ctx) {
            found.insert(child.to_path_buf(), matched);
        }
        return;
    }
    if name != Some(NODE_MODULES) {
        walk(child, depth + 1, ctx, found);
    }
}

fn should_offer(path: &Path, name: Option<&str>, ctx: &ScanContext) -> bool {
    if is_excluded(path, ctx) {
        return false;
    }
    let stale_days = ctx.config.stale_node_modules_days;
    name != Some(NODE_MODULES) || node_modules_is_stale(path, stale_days)
}

fn kind_label(path: &Path) -> &'static str {
    match path.file_name().and_then(|name| name.to_str()) {
        Some("target") => "Rust target",
        Some(".next") => "Next.js build",
        Some(".turbo") => "Turborepo cache",
        Some(NODE_MODULES) => "stale node_modules",
        _ => "Python virtualenv",
    }
}

fn describe(path: &Path, category: Category, bytes: u64) -> String {
    let modified = fs::metadata(path)
        .and_then(|meta| meta.modified())
        .map_or_else(
            |_| "unknown".to_string(),
            |time| format_age(secs_since(time)),
        );
    let base = format!(
        "{} · {} · last modified {modified}",
        kind_label(path),
        format_bytes(bytes)
    );
    match category {
        Category::PythonEnvs => format!("{base} · {VENV_HINT}"),
        _ => base,
    }
}

fn build_item(path: &Path, category: Category, risk: Risk, home: &Path) -> Item {
    let bytes = disk_usage(&[path.to_path_buf()]);
    Item {
        category,
        label: tilde_path(path, home),
        detail: describe(path, category, bytes),
        bytes,
        risk,
        action: Action::DeletePath(path.to_path_buf()),
        preselected: risk == Risk::Safe,
        typed_confirmation: None,
    }
}

pub fn scan(ctx: &ScanContext) -> Vec<Item> {
    let mut found = Found::new();
    for root in &ctx.config.dev_roots {
        walk(root, 0, ctx, &mut found);
    }
    found
        .par_iter()
        .map(|(path, (category, risk))| build_item(path, *category, *risk, &ctx.home))
        .collect()
}
