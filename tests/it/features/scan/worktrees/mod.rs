mod assess;
mod discover;
mod prune;
mod scan;

use crate::support::fixtures::test_ctx;
use crate::support::git::GIT;
use anyhow::{Result, anyhow};
use diskdiet::domain::model::Item;
use diskdiet::features::scan::worktrees::{Assessment, assess_worktree};
use std::path::{Path, PathBuf};

fn inspect_worktree(worktree: &Path) -> Result<Assessment> {
    assess_worktree(Path::new(GIT), worktree, Some("main")).ok_or_else(|| anyhow!("inspect failed"))
}

fn scan_items(home: &Path, roots: Vec<PathBuf>) -> Vec<Item> {
    diskdiet::features::scan::worktrees::scan(&test_ctx(home, roots))
}

fn scan_root(root: &Path) -> Vec<Item> {
    scan_items(root, vec![root.to_path_buf()])
}

fn item_for<'a>(items: &'a [Item], branch: &str) -> Result<&'a Item> {
    let found = items.iter().find(|item| item.label == branch);
    found.ok_or_else(|| anyhow!("no item labelled {branch}"))
}
