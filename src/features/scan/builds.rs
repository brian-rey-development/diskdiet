use super::ScanContext;
use super::support::{DETAIL_SEPARATOR, GIT_DIR};
use crate::domain::model::{Action, Category, Item, Risk};
use crate::infra::fs::paths::child_dirs;
use crate::infra::fs::size::{format_bytes, path_usage};
use crate::infra::util::fmt::{format_age, tilde_path};
use crate::infra::util::time::{SECS_PER_DAY, secs_since};
use rayon::prelude::*;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

const BUILD_SCAN_MAX_DEPTH: usize = 12;
const NODE_MODULES: &str = "node_modules";
const PACKAGE_JSON: &str = "package.json";
const LINKED_WORKTREE_MARKER: &str = "/worktrees/";
const UNKNOWN_AGE: &str = "unknown";

enum Evidence {
    SiblingManifest(&'static [&'static str]),
    ContainedFile(&'static str),
}

struct BuildKind {
    dir_name: &'static str,
    evidence: Evidence,
    category: Category,
    risk: Risk,
    label: &'static str,
    hint: Option<&'static str>,
    offered_only_when_stale: bool,
}

const BUILD_KINDS: &[BuildKind] = &[
    BuildKind {
        dir_name: "target",
        evidence: Evidence::SiblingManifest(&["Cargo.toml"]),
        category: Category::BuildArtifacts,
        risk: Risk::Safe,
        label: "Rust target",
        hint: None,
        offered_only_when_stale: false,
    },
    BuildKind {
        dir_name: ".next",
        evidence: Evidence::SiblingManifest(&[PACKAGE_JSON]),
        category: Category::BuildArtifacts,
        risk: Risk::Safe,
        label: "Next.js build",
        hint: None,
        offered_only_when_stale: false,
    },
    BuildKind {
        dir_name: ".turbo",
        evidence: Evidence::SiblingManifest(&[PACKAGE_JSON, "turbo.json"]),
        category: Category::BuildArtifacts,
        risk: Risk::Safe,
        label: "Turborepo cache",
        hint: None,
        offered_only_when_stale: false,
    },
    BuildKind {
        dir_name: NODE_MODULES,
        evidence: Evidence::SiblingManifest(&[PACKAGE_JSON]),
        category: Category::BuildArtifacts,
        risk: Risk::Review,
        label: "stale node_modules",
        hint: None,
        offered_only_when_stale: true,
    },
    BuildKind {
        dir_name: ".venv",
        evidence: Evidence::ContainedFile("pyvenv.cfg"),
        category: Category::PythonEnvs,
        risk: Risk::Review,
        label: "Python virtualenv",
        hint: Some("run `uv sync` to recreate"),
        offered_only_when_stale: false,
    },
];

type Found = BTreeMap<PathBuf, &'static BuildKind>;

impl Evidence {
    fn holds_for(&self, dir: &Path) -> bool {
        match self {
            Self::SiblingManifest(names) => dir
                .parent()
                .is_some_and(|parent| names.iter().any(|name| parent.join(name).is_file())),
            Self::ContainedFile(name) => dir.join(name).is_file(),
        }
    }
}

fn is_real_dir(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok_and(|meta| meta.is_dir())
}

fn matching_kind(dir: &Path) -> Option<&'static BuildKind> {
    if !is_real_dir(dir) {
        return None;
    }
    let name = dir.file_name()?.to_str()?;
    BUILD_KINDS
        .iter()
        .find(|kind| kind.dir_name == name && kind.evidence.holds_for(dir))
}

pub fn classify(dir: &Path) -> Option<(Category, Risk)> {
    matching_kind(dir).map(|kind| (kind.category, kind.risk))
}

fn is_linked_worktree(dir: &Path) -> bool {
    fs::read_to_string(dir.join(GIT_DIR))
        .is_ok_and(|text| text.starts_with("gitdir:") && text.contains(LINKED_WORKTREE_MARKER))
}

fn modified_time(path: &Path) -> Option<SystemTime> {
    fs::metadata(path).and_then(|meta| meta.modified()).ok()
}

fn node_modules_is_stale(dir: &Path, stale_days: u64) -> bool {
    let manifest = dir
        .parent()
        .and_then(|parent| modified_time(&parent.join(PACKAGE_JSON)));
    let newest = manifest.zip(modified_time(dir)).map(|(a, b)| a.max(b));
    newest.is_some_and(|time| secs_since(time) >= stale_days.saturating_mul(SECS_PER_DAY))
}

fn is_excluded(dir: &Path, ctx: &ScanContext) -> bool {
    ctx.config
        .exclude
        .iter()
        .any(|excluded| dir.starts_with(excluded))
}

fn walk(dir: &Path, depth: usize, ctx: &ScanContext, found: &mut Found) {
    if depth > BUILD_SCAN_MAX_DEPTH || is_excluded(dir, ctx) || is_linked_worktree(dir) {
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
    if let Some(kind) = matching_kind(child) {
        if should_offer(child, kind, ctx) {
            found.insert(child.to_path_buf(), kind);
        }
        return;
    }
    if name != Some(NODE_MODULES) {
        walk(child, depth + 1, ctx, found);
    }
}

fn should_offer(path: &Path, kind: &BuildKind, ctx: &ScanContext) -> bool {
    if is_excluded(path, ctx) {
        return false;
    }
    let stale_days = ctx.config.stale_node_modules_days;
    !kind.offered_only_when_stale || node_modules_is_stale(path, stale_days)
}

fn modified_text(path: &Path) -> String {
    fs::metadata(path)
        .and_then(|meta| meta.modified())
        .map_or_else(
            |_| UNKNOWN_AGE.to_owned(),
            |time| format_age(secs_since(time)),
        )
}

fn describe(path: &Path, kind: &BuildKind, bytes: u64) -> String {
    let mut parts = vec![
        kind.label.to_owned(),
        format_bytes(bytes),
        format!("last modified {}", modified_text(path)),
    ];
    parts.extend(kind.hint.map(str::to_owned));
    parts.join(DETAIL_SEPARATOR)
}

fn build_item(path: &Path, kind: &BuildKind, home: &Path) -> Item {
    let bytes = path_usage(path);
    Item::new(
        kind.category,
        tilde_path(path, home),
        describe(path, kind, bytes),
        bytes,
        kind.risk,
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
        .map(|(path, kind)| build_item(path, kind, &ctx.home))
        .collect()
}
