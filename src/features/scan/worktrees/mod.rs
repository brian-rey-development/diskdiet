mod assess;
mod discover;
mod items;

pub use assess::{Assessment, assess_worktree, base_branch, risk_from_counts};
pub use discover::find_repos;

use super::ScanContext;
use crate::domain::model::Item;
use crate::infra::fs::paths::canonical_or_self;
use crate::infra::git::{
    self, porcelain::WorktreeEntry, porcelain::parse_porcelain, run as run_git,
};
use crate::infra::sys::platform::volume_is_mounted;
use items::{prune_item, worktree_item};
use rayon::prelude::*;
use std::path::{Path, PathBuf};

struct RepoScanEnv {
    git: PathBuf,
    home: PathBuf,
}

fn folder_is_missing(entry: &WorktreeEntry) -> bool {
    matches!(entry.path.try_exists(), Ok(false)) && volume_is_mounted(&entry.path)
}

fn linked_entries(listing: &str, repo: &Path) -> Vec<WorktreeEntry> {
    parse_porcelain(listing)
        .into_iter()
        .filter(|entry| !entry.bare && !entry.locked && canonical_or_self(&entry.path) != repo)
        .collect()
}

fn present_items(repo: &Path, present: &[WorktreeEntry], env: &RepoScanEnv) -> Vec<Item> {
    if present.is_empty() {
        return Vec::new();
    }
    let base = base_branch(&env.git, repo);
    present
        .par_iter()
        .filter_map(|entry| worktree_item(repo, entry, base.as_deref(), env))
        .collect()
}

fn scan_repo(repo: &Path, env: &RepoScanEnv) -> Vec<Item> {
    if !discover::has_linked_worktree_records(repo) {
        return Vec::new();
    }
    let Some(listing) = run_git(&env.git, repo, &["worktree", "list", "--porcelain"]) else {
        return Vec::new();
    };
    let linked = linked_entries(&listing, repo);
    let unplugged = linked.iter().any(|entry| !volume_is_mounted(&entry.path));
    let (missing, present): (Vec<_>, Vec<_>) = linked.into_iter().partition(folder_is_missing);
    let mut items = present_items(repo, &present, env);
    if !unplugged {
        items.extend(prune_item(repo, missing.len(), &env.home));
    }
    items
}

pub fn scan(ctx: &ScanContext) -> Vec<Item> {
    let env = RepoScanEnv {
        git: git::program(&ctx.tools).to_path_buf(),
        home: ctx.home.clone(),
    };
    find_repos(&ctx.config.dev_roots, &ctx.config.exclude)
        .par_iter()
        .flat_map(|repo| scan_repo(repo, &env))
        .collect()
}
