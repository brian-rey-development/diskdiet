use super::{inspect_worktree, item_for, scan_items, scan_root};
use crate::support::fixtures::{temp_root, write_file};
use crate::support::git::{GIT, commit_file, git, init_repo, repo_with_worktree};
use anyhow::Result;
use diskdiet::domain::model::{Action, Removal, Risk};
use diskdiet::features::scan::worktrees::{base_branch, risk_from_counts};
use std::path::Path;

fn add_real_submodule(root: &Path, worktree: &Path) -> Result<()> {
    let lib = root.join("lib");
    init_repo(&lib)?;
    let source = lib.to_string_lossy().into_owned();
    let allow = ["-c", "protocol.file.allow=always"];
    let add = ["submodule", "add", "-q", &source, "lib"];
    git(worktree, &[&allow[..], &add[..]].concat())?;
    git(worktree, &["commit", "-m", "add submodule"]).map(drop)
}

#[test]
fn zero_commits_ahead_with_uncommitted_changes_is_danger() -> Result<()> {
    let (_tmp, root) = temp_root()?;
    let (_repo, wt) = repo_with_worktree(&root, "feature")?;
    write_file(&wt.join("notes.txt"), "unsaved thoughts")?;
    let assessment = inspect_worktree(&wt)?;
    assert_eq!((assessment.ahead, assessment.dirty), (Some(0), 1));
    assert_eq!(assessment.risk(), Risk::Danger);
    Ok(())
}

#[test]
fn ignored_env_file_alone_makes_a_worktree_danger() -> Result<()> {
    let (_tmp, root) = temp_root()?;
    let (_repo, wt) = repo_with_worktree(&root, "feature")?;
    write_file(&wt.join(".env"), "SECRET=1")?;
    let assessment = inspect_worktree(&wt)?;
    assert_eq!((assessment.dirty, assessment.risky_ignored), (0, 1));
    assert_eq!(assessment.risk(), Risk::Danger);
    Ok(())
}

#[test]
fn ignored_personal_folder_makes_a_worktree_danger() -> Result<()> {
    let (_tmp, root) = temp_root()?;
    let (repo, wt) = repo_with_worktree(&root, "feature")?;
    write_file(&repo.join(".git/info/exclude"), "local-data/\n")?;
    write_file(&wt.join("local-data/app.sqlite"), "rows")?;
    let assessment = inspect_worktree(&wt)?;
    assert_eq!((assessment.dirty, assessment.risky_ignored), (0, 1));
    assert_eq!(assessment.risk(), Risk::Danger);
    Ok(())
}

#[test]
fn ignored_build_output_alone_keeps_a_worktree_safe() -> Result<()> {
    let (_tmp, root) = temp_root()?;
    let (_repo, wt) = repo_with_worktree(&root, "feature")?;
    write_file(&wt.join("target/debug/app"), "binary")?;
    write_file(&wt.join("node_modules/dep/index.js"), "module")?;
    let assessment = inspect_worktree(&wt)?;
    assert_eq!((assessment.dirty, assessment.risky_ignored), (0, 0));
    assert_eq!(assessment.risk(), Risk::Safe);
    Ok(())
}

#[test]
fn commits_ahead_with_a_clean_tree_is_review() -> Result<()> {
    let (_tmp, root) = temp_root()?;
    let (_repo, wt) = repo_with_worktree(&root, "feature")?;
    commit_file(&wt, "feature.txt", "work", "add feature")?;
    let assessment = inspect_worktree(&wt)?;
    assert_eq!((assessment.ahead, assessment.dirty), (Some(1), 0));
    assert_eq!(assessment.risk(), Risk::Review);
    let items = scan_root(&root);
    let item = item_for(&items, "feature")?;
    assert_eq!((item.risk, item.preselected), (Risk::Review, false));
    Ok(())
}

#[test]
fn base_branch_falls_back_to_the_first_known_default() -> Result<()> {
    let (_tmp, root) = temp_root()?;
    let repo = root.join("repo");
    init_repo(&repo)?;
    assert_eq!(
        base_branch(Path::new(GIT), &repo).as_deref(),
        Some("refs/heads/main")
    );
    Ok(())
}

#[test]
fn assess_combines_ahead_dirty_and_ignored_counts() {
    assert_eq!(risk_from_counts(0, 0, 0), Risk::Safe);
    assert_eq!(risk_from_counts(3, 0, 0), Risk::Review);
    assert_eq!(risk_from_counts(0, 1, 0), Risk::Danger);
    assert_eq!(risk_from_counts(0, 0, 1), Risk::Danger);
    assert_eq!(risk_from_counts(5, 2, 1), Risk::Danger);
}

#[test]
fn skip_worktree_edits_make_a_worktree_danger() -> Result<()> {
    let (_tmp, root) = temp_root()?;
    let (_repo, wt) = repo_with_worktree(&root, "feature")?;
    git(&wt, &["update-index", "--skip-worktree", "README.md"])?;
    write_file(&wt.join("README.md"), "local only edit")?;
    let assessment = inspect_worktree(&wt)?;
    assert_eq!(assessment.dirty, 1);
    assert_eq!(assessment.risk(), Risk::Danger);
    Ok(())
}

#[test]
fn worktrees_with_submodules_are_danger_because_git_needs_force() -> Result<()> {
    let (_tmp, root) = temp_root()?;
    let (_repo, wt) = repo_with_worktree(&root, "feature")?;
    add_real_submodule(&root, &wt)?;
    let assessment = inspect_worktree(&wt)?;
    assert!(assessment.has_submodules);
    assert_eq!(assessment.risk(), Risk::Danger);
    let items = scan_items(&root, vec![root.join("repo")]);
    let item = item_for(&items, "feature")?;
    assert!(matches!(
        item.action,
        Action::RemoveWorktree {
            removal: Removal::Forced,
            ..
        }
    ));
    Ok(())
}
