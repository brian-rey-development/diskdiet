use crate::support::fixtures::{fixture_item, temp_root};
use crate::support::git::repo_with_worktree;
use anyhow::Result;
use diskdiet::domain::model::{Action, Outcome, Removal, RunMode};
use diskdiet::features::actions::execute;
use diskdiet::infra::sys::tools::Tools;
use std::fs;
use std::path::{Path, PathBuf};

const FORCED: Removal = Removal::Forced {
    dirty: 1,
    risky_ignored: 0,
};

struct DryFixture {
    dir: PathBuf,
    file: PathBuf,
    marker: PathBuf,
    repo: PathBuf,
    worktree: PathBuf,
}

fn dry_fixture(root: &Path) -> Result<DryFixture> {
    let dir = root.join("victim-dir");
    let file = root.join("victim.txt");
    fs::create_dir(&dir)?;
    fs::write(&file, "x")?;
    let (repo, worktree) = repo_with_worktree(root, "feature")?;
    Ok(DryFixture {
        dir,
        file,
        marker: root.join("marker"),
        repo,
        worktree,
    })
}

fn every_variant(fx: &DryFixture) -> Vec<Action> {
    vec![
        Action::DeletePath(fx.dir.clone()),
        Action::DeletePath(fx.file.clone()),
        Action::MoveToTrash(fx.file.clone()),
        Action::EmptyTrash,
        Action::RunCommand {
            program: PathBuf::from("/usr/bin/touch"),
            args: vec![fx.marker.to_string_lossy().to_string()],
        },
        Action::RemoveWorktree {
            repo: fx.repo.clone(),
            worktree: fx.worktree.clone(),
            removal: FORCED,
        },
        Action::PruneWorktrees {
            repo: fx.repo.clone(),
        },
    ]
}

#[test]
fn dry_run_never_touches_files_for_every_variant() -> Result<()> {
    let (_tmp, root) = temp_root()?;
    let fx = dry_fixture(&root)?;
    for action in every_variant(&fx) {
        let outcome = execute(&fixture_item(action), &Tools::default(), RunMode::DryRun);
        assert_eq!(outcome, Outcome::DryRun);
    }
    assert!(fx.dir.exists() && fx.file.exists() && fx.worktree.exists());
    assert!(!fx.marker.exists());
    Ok(())
}
