use crate::support::fixtures::temp_root;
use crate::support::git::{branch_exists, git, repo_with_worktree};
use anyhow::{Result, anyhow};
use diskdiet::domain::model::{Action, Outcome, Removal};
use diskdiet::features::actions::perform;
use diskdiet::infra::sys::tools::Tools;
use std::fs;
use std::path::{Path, PathBuf};

const CONFIRMED_ONE_DIRTY: Removal = Removal::Forced {
    dirty: 1,
    risky_ignored: 0,
};

fn remove(repo: &Path, worktree: &Path, removal: Removal) -> Outcome {
    let action = Action::RemoveWorktree {
        repo: repo.to_path_buf(),
        worktree: worktree.to_path_buf(),
        removal,
    };
    perform(&action, &Tools::default())
}

fn prune(repo: &Path) -> Outcome {
    let action = Action::PruneWorktrees {
        repo: repo.to_path_buf(),
    };
    perform(&action, &Tools::default())
}

#[test]
fn remove_worktree_removes_the_folder_and_keeps_the_branch() -> Result<()> {
    let (_tmp, root) = temp_root()?;
    let (repo, worktree) = repo_with_worktree(&root, "feature")?;
    assert_eq!(remove(&repo, &worktree, Removal::Rechecked), Outcome::Done);
    assert!(!worktree.exists());
    assert!(branch_exists(&repo, "feature")?);
    Ok(())
}

#[test]
fn unconfirmed_worktree_that_changed_since_the_scan_is_kept() -> Result<()> {
    let (_tmp, root) = temp_root()?;
    let (repo, worktree) = repo_with_worktree(&root, "feature")?;
    fs::write(worktree.join("scratch.txt"), "written after the scan")?;
    let outcome = remove(&repo, &worktree, Removal::Rechecked);
    assert!(matches!(outcome, Outcome::Skipped(_)));
    assert!(worktree.join("scratch.txt").exists());
    Ok(())
}

#[test]
fn typed_confirmation_removes_a_dirty_worktree() -> Result<()> {
    let (_tmp, root) = temp_root()?;
    let (repo, worktree) = repo_with_worktree(&root, "feature")?;
    fs::write(worktree.join("scratch.txt"), "uncommitted")?;
    assert_eq!(remove(&repo, &worktree, CONFIRMED_ONE_DIRTY), Outcome::Done);
    assert!(!worktree.exists());
    Ok(())
}

#[test]
fn typed_confirmation_is_void_when_more_files_changed_after_it() -> Result<()> {
    let (_tmp, root) = temp_root()?;
    let (repo, worktree) = repo_with_worktree(&root, "feature")?;
    fs::write(worktree.join("scratch.txt"), "uncommitted")?;
    fs::write(worktree.join("later.txt"), "written after the confirmation")?;
    let outcome = remove(&repo, &worktree, CONFIRMED_ONE_DIRTY);
    assert!(matches!(outcome, Outcome::Skipped(_)), "{outcome:?}");
    assert!(worktree.join("later.txt").exists());
    Ok(())
}

#[test]
fn prune_is_skipped_when_it_would_forget_a_worktree_on_an_unplugged_drive() -> Result<()> {
    let (_tmp, root) = temp_root()?;
    let (repo, worktree) = repo_with_worktree(&root, "feature")?;
    fs::remove_dir_all(&worktree)?;
    let name = worktree.file_name().ok_or_else(|| anyhow!("name"))?;
    let gitdir = repo.join(".git/worktrees").join(name).join("gitdir");
    fs::write(&gitdir, "/Volumes/diskdiet-test-unplugged/wt/.git\n")?;
    assert!(matches!(prune(&repo), Outcome::Skipped(_)));
    assert!(gitdir.exists());
    Ok(())
}

#[test]
fn prune_worktrees_clears_entries_whose_folder_is_gone() -> Result<()> {
    let (_tmp, root) = temp_root()?;
    let (repo, worktree) = repo_with_worktree(&root, "feature")?;
    fs::remove_dir_all(&worktree)?;
    assert_eq!(prune(&repo), Outcome::Done);
    assert!(!git(&repo, &["worktree", "list"])?.contains("wt-feature"));
    assert!(branch_exists(&repo, "feature")?);
    Ok(())
}

#[test]
fn prune_is_skipped_when_the_worktree_listing_fails() {
    let missing = PathBuf::from("/nonexistent-diskdiet-repo");
    assert!(matches!(prune(&missing), Outcome::Skipped(_)));
}

#[test]
fn prune_is_skipped_for_a_locked_worktree_on_an_unplugged_drive() -> Result<()> {
    let (_tmp, root) = temp_root()?;
    let (repo, worktree) = repo_with_worktree(&root, "feature")?;
    git(&repo, &["worktree", "lock", &worktree.to_string_lossy()])?;
    fs::remove_dir_all(&worktree)?;
    let name = worktree.file_name().ok_or_else(|| anyhow!("name"))?;
    let gitdir = repo.join(".git/worktrees").join(name).join("gitdir");
    fs::write(&gitdir, "/Volumes/diskdiet-test-unplugged/wt/.git\n")?;
    assert!(matches!(prune(&repo), Outcome::Skipped(_)));
    assert!(gitdir.exists());
    Ok(())
}

#[test]
fn remove_worktree_refuses_a_path_the_guard_rejects() -> Result<()> {
    let (_tmp, root) = temp_root()?;
    let (repo, _worktree) = repo_with_worktree(&root, "feature")?;
    let outcome = remove(&repo, Path::new("relative/wt"), Removal::Rechecked);
    assert!(matches!(outcome, Outcome::Failed(_)), "{outcome:?}");
    Ok(())
}
