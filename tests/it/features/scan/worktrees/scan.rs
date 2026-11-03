use super::{item_for, scan_items, scan_root};
use crate::support::fixtures::{temp_root, write_file};
use crate::support::git::{add_worktree, git, init_repo, repo_with_worktree};
use anyhow::{Result, anyhow};
use diskdiet::domain::model::{Action, Category, Removal, Risk};
use diskdiet::infra::fs::size::format_bytes;
use std::fs;

const PAYLOAD_BYTES: usize = 200_000;

#[test]
fn danger_worktree_requires_typed_delete_and_is_not_preselected() -> Result<()> {
    let (_tmp, root) = temp_root()?;
    let (_repo, wt) = repo_with_worktree(&root, "feature")?;
    write_file(&wt.join("notes.txt"), "unsaved thoughts")?;
    let items = scan_root(&root);
    let item = item_for(&items, "feature")?;
    assert_eq!(item.risk, Risk::Danger);
    assert_eq!(item.typed_confirmation.as_deref(), Some("delete"));
    assert!(!item.preselected);
    assert_eq!(item.category, Category::AgentWorktrees);
    Ok(())
}

#[test]
fn safe_worktree_is_preselected_in_scan() -> Result<()> {
    let (_tmp, root) = temp_root()?;
    let (repo, wt) = repo_with_worktree(&root, "feature")?;
    write_file(&wt.join("target/debug/app"), "binary")?;
    let items = scan_root(&root);
    let item = item_for(&items, "feature")?;
    assert_eq!((item.risk, item.preselected), (Risk::Safe, true));
    assert!(item.typed_confirmation.is_none());
    assert_eq!(
        item.action,
        Action::RemoveWorktree {
            repo: fs::canonicalize(repo)?,
            worktree: wt,
            removal: Removal::Rechecked,
        }
    );
    assert!(item.detail.starts_with("merged · 0 ahead"));
    Ok(())
}

#[test]
fn worktrees_outside_the_dev_roots_are_followed() -> Result<()> {
    let (_tmp, root) = temp_root()?;
    let (_elsewhere, outside) = temp_root()?;
    let repo = root.join("dev/repo");
    init_repo(&repo)?;
    add_worktree(&repo, &outside.join("orca/workspaces/feature"), "feature")?;
    let items = scan_items(&root, vec![root.join("dev")]);
    assert_eq!(item_for(&items, "feature")?.risk, Risk::Safe);
    Ok(())
}

#[test]
fn main_and_detached_worktrees_are_not_offered() -> Result<()> {
    let (_tmp, root) = temp_root()?;
    let repo = root.join("repo");
    init_repo(&repo)?;
    let detached = root.join("wt-detached");
    git(
        &repo,
        &["worktree", "add", "--detach", &detached.to_string_lossy()],
    )?;
    assert_eq!(scan_root(&root), [] as [diskdiet::domain::model::Item; 0]);
    Ok(())
}

#[test]
fn repos_without_linked_worktrees_yield_nothing() -> Result<()> {
    let (_tmp, root) = temp_root()?;
    init_repo(&root.join("plain"))?;
    assert_eq!(scan_root(&root), [] as [diskdiet::domain::model::Item; 0]);
    Ok(())
}

#[test]
fn worktree_item_reports_the_measured_size_and_the_inspected_risk() -> Result<()> {
    let (_tmp, root) = temp_root()?;
    let (_repo, worktree) = repo_with_worktree(&root, "big")?;
    write_file(&worktree.join("notes.txt"), &"x".repeat(PAYLOAD_BYTES))?;
    let items = scan_root(&root);
    let item = item_for(&items, "big").map_err(|_| anyhow!("no item for the worktree"))?;
    assert!(item.bytes >= PAYLOAD_BYTES as u64);
    assert!(item.detail.contains(&format_bytes(item.bytes)));
    assert!(item.detail.contains("1 uncommitted file"));
    assert_eq!(item.risk, Risk::Danger);
    Ok(())
}
