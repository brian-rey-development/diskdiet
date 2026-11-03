mod common;

use anyhow::{Result, anyhow};
use common::{add_worktree, git, init_repo, temp_root, test_ctx, write_file};
use diskdiet::model::{Action, Category, DELETE_CONFIRMATION, Item, Risk};
use diskdiet::scan::inherit_child_risk;
use diskdiet::scan::worktrees::scan;
use diskdiet::size::format_bytes;
use std::fs;
use std::path::{Path, PathBuf};

const PAYLOAD_BYTES: usize = 200_000;

fn scan_items(root: &Path) -> Vec<Item> {
    scan(&test_ctx(root, vec![root.to_path_buf()]))
}

fn delete_item(path: &Path, risk: Risk) -> Item {
    let action = Action::DeletePath(path.to_path_buf());
    Item::new(Category::BuildArtifacts, "item", "detail", 1, risk, action)
}

#[test]
fn the_only_worktree_folder_being_deleted_still_yields_the_prune_item() -> Result<()> {
    let (_tmp, root) = temp_root()?;
    let repo = root.join("repo");
    init_repo(&repo)?;
    let worktree = add_worktree(&repo, &root.join("wt-only"), "only")?;
    fs::remove_dir_all(&worktree)?;
    let items = scan_items(&root);
    assert_eq!(items.len(), 1);
    assert_eq!(
        items[0].action,
        Action::PruneWorktrees {
            repo: fs::canonicalize(&repo)?
        }
    );
    assert!(
        items[0]
            .detail
            .starts_with("1 worktree folder no longer exists")
    );
    Ok(())
}

#[test]
fn repos_without_linked_worktrees_yield_nothing() -> Result<()> {
    let (_tmp, root) = temp_root()?;
    init_repo(&root.join("plain"))?;
    assert!(scan_items(&root).is_empty());
    Ok(())
}

#[test]
fn a_repo_whose_worktree_was_removed_cleanly_yields_nothing() -> Result<()> {
    let (_tmp, root) = temp_root()?;
    let repo = root.join("repo");
    init_repo(&repo)?;
    let worktree = add_worktree(&repo, &root.join("wt-done"), "done")?;
    git(&repo, &["worktree", "remove", &worktree.to_string_lossy()])?;
    assert!(scan_items(&root).is_empty());
    Ok(())
}

#[test]
fn present_and_missing_worktrees_in_one_repo_are_both_reported() -> Result<()> {
    let (_tmp, root) = temp_root()?;
    let repo = root.join("repo");
    init_repo(&repo)?;
    add_worktree(&repo, &root.join("wt-kept"), "kept")?;
    let gone = add_worktree(&repo, &root.join("wt-gone"), "gone")?;
    fs::remove_dir_all(gone)?;
    let items = scan_items(&root);
    let labels: Vec<&str> = items.iter().map(|item| item.label.as_str()).collect();
    assert_eq!(items.len(), 2);
    assert!(labels.contains(&"kept"));
    assert!(labels.contains(&"Stale worktree records in repo"));
    Ok(())
}

#[test]
fn worktree_item_reports_the_measured_size_and_the_inspected_risk() -> Result<()> {
    let (_tmp, root) = temp_root()?;
    let repo = root.join("repo");
    init_repo(&repo)?;
    let worktree = add_worktree(&repo, &root.join("wt-big"), "big")?;
    write_file(&worktree.join("notes.txt"), &"x".repeat(PAYLOAD_BYTES))?;
    let items = scan_items(&root);
    let item = items
        .iter()
        .find(|item| item.label == "big")
        .ok_or_else(|| anyhow!("no item for the worktree"))?;
    assert!(item.bytes >= PAYLOAD_BYTES as u64);
    assert!(item.detail.contains(&format_bytes(item.bytes)));
    assert!(item.detail.contains("1 uncommitted file"));
    assert_eq!(item.risk, Risk::Danger);
    Ok(())
}

#[test]
fn a_danger_child_makes_its_safe_parent_danger_with_typed_confirmation() {
    let parent = PathBuf::from("/home/me/project");
    let mut items = vec![
        delete_item(&parent, Risk::Safe),
        delete_item(&parent.join("wt"), Risk::Danger),
    ];
    items[0].preselected = true;
    inherit_child_risk(&mut items);
    assert_eq!(items[0].risk, Risk::Danger);
    assert!(!items[0].preselected);
    assert_eq!(
        items[0].typed_confirmation.as_deref(),
        Some(DELETE_CONFIRMATION)
    );
    assert_eq!(items[1].risk, Risk::Danger);
}

#[test]
fn equal_or_lower_child_risk_leaves_the_parent_untouched() {
    let parent = PathBuf::from("/home/me/project");
    let mut items = vec![
        delete_item(&parent, Risk::Review),
        delete_item(&parent.join("a"), Risk::Review),
        delete_item(&parent.join("b"), Risk::Safe),
    ];
    inherit_child_risk(&mut items);
    assert_eq!(items[0].risk, Risk::Review);
    assert!(items[0].typed_confirmation.is_none());
}

#[test]
fn a_prune_record_inside_a_deleted_tree_does_not_raise_its_risk() {
    let parent = PathBuf::from("/home/me/project");
    let prune = Item::new(
        Category::AgentWorktrees,
        "prune",
        "detail",
        0,
        Risk::Danger,
        Action::PruneWorktrees {
            repo: parent.join("repo"),
        },
    );
    let mut items = vec![delete_item(&parent, Risk::Safe), prune];
    inherit_child_risk(&mut items);
    assert_eq!(items[0].risk, Risk::Safe);
}
