use crate::support::fixtures::temp_root;
use crate::support::git::{branch_exists, git, repo_with_worktree};
use anyhow::{Result, anyhow};
use diskdiet::actions::{perform, prune_names};
use diskdiet::model::{Action, Outcome, Removal};
use diskdiet::tools::Tools;
use std::fs;
use std::path::Path;

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
    assert_eq!(remove(&repo, &worktree, Removal::Forced), Outcome::Done);
    assert!(!worktree.exists());
    Ok(())
}

#[test]
fn prune_preview_lines_name_the_worktrees_git_would_forget() {
    let stderr = "Removing worktrees/gone: gitdir file points to non-existent location\n\
                  Removing worktrees/old one: not a valid directory\n\
                  something else\n";
    assert_eq!(prune_names(stderr), vec!["gone", "old one"]);
}

#[test]
fn prune_preview_lines_without_the_prefix_are_ignored() {
    let output = "Removing worktrees/a: gitdir file points to non-existent location\nnoise\n";
    assert_eq!(prune_names(output), vec!["a".to_owned()]);
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
