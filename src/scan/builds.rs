use super::ScanContext;
use crate::fmt::{format_age, tilde_path};
use crate::fsutil::child_dirs;
use crate::model::{Action, Category, Item, Risk};
use crate::size::{disk_usage, format_bytes};
use crate::time::{SECS_PER_DAY, secs_since};
use rayon::prelude::*;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

const PYVENV_CONFIG: &str = "pyvenv.cfg";
const MAX_WALK_DEPTH: usize = 12;
const GIT_DIR: &str = ".git";
const NODE_MODULES: &str = "node_modules";
const PACKAGE_JSON: &str = "package.json";
const LINKED_WORKTREE_MARKER: &str = "/worktrees/";
const VENV_HINT: &str = "run `uv sync` to recreate";

type Found = BTreeMap<PathBuf, (Category, Risk)>;

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
    Item::new(
        category,
        tilde_path(path, home),
        describe(path, category, bytes),
        bytes,
        risk,
        Action::DeletePath(path.to_path_buf()),
    )
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
