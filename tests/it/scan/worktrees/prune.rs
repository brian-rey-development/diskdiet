use super::{item_for, scan_root};
use crate::support::fixtures::{temp_root, write_file};
use crate::support::git::{add_worktree, git, init_repo, repo_with_worktree};
use anyhow::Result;
use diskdiet::model::{Action, Risk};
use std::fs;

#[test]
fn deleted_worktree_folders_yield_one_prune_item_per_repo() -> Result<()> {
    let (_tmp, root) = temp_root()?;
    let (repo, first) = repo_with_worktree(&root, "first")?;
    let second = add_worktree(&repo, &root.join("wt-second"), "second")?;
    fs::remove_dir_all(&first)?;
    fs::remove_dir_all(&second)?;
    let items = scan_root(&root);
    assert_eq!(items.len(), 1);
    assert_eq!(
        items[0].action,
        Action::PruneWorktrees {
            repo: fs::canonicalize(repo)?
        }
    );
    assert_eq!(
        (items[0].bytes, items[0].risk, items[0].preselected),
        (0, Risk::Safe, true)
    );
    Ok(())
}

#[test]
fn no_prune_is_offered_while_a_worktree_sits_on_an_unplugged_volume() -> Result<()> {
    let (_tmp, root) = temp_root()?;
    let (repo, first) = repo_with_worktree(&root, "first")?;
    let unplugged = add_worktree(&repo, &root.join("wt-second"), "second")?;
    fs::remove_dir_all(&first)?;
    fs::remove_dir_all(&unplugged)?;
    let gitdir = repo.join(".git/worktrees/wt-second/gitdir");
    write_file(&gitdir, "/Volumes/diskdiet-test-unplugged/wt-second/.git\n")?;
    assert!(scan_root(&root).is_empty());
    Ok(())
}

#[test]
fn the_only_worktree_folder_being_deleted_still_yields_the_prune_item() -> Result<()> {
    let (_tmp, root) = temp_root()?;
    let (repo, worktree) = repo_with_worktree(&root, "only")?;
    fs::remove_dir_all(&worktree)?;
    let items = scan_root(&root);
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
fn a_repo_whose_worktree_was_removed_cleanly_yields_nothing() -> Result<()> {
    let (_tmp, root) = temp_root()?;
    let (repo, worktree) = repo_with_worktree(&root, "done")?;
    git(&repo, &["worktree", "remove", &worktree.to_string_lossy()])?;
    assert!(scan_root(&root).is_empty());
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
    let items = scan_root(&root);
    assert_eq!(items.len(), 2);
    assert!(item_for(&items, "kept").is_ok());
    assert!(item_for(&items, "Stale worktree records in repo").is_ok());
    Ok(())
}
