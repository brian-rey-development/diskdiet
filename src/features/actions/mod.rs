mod command;
mod delete;
mod trash;
mod worktree;

use crate::domain::model::{Action, Item, Outcome, RunMode};
use crate::domain::safety::is_safe_delete_target;
use crate::infra::sys::log;
use crate::infra::sys::platform;
use crate::infra::sys::tools::Tools;
use std::io;
use std::path::Path;

pub use trash::{applescript_escape, trash_script};

pub fn execute(item: &Item, tools: &Tools, mode: RunMode) -> Outcome {
    if mode == RunMode::DryRun {
        return Outcome::DryRun;
    }
    let outcome = perform(&item.action, tools);
    record_outcome(item, &outcome);
    outcome
}

pub fn perform(action: &Action, tools: &Tools) -> Outcome {
    match action {
        Action::DeletePath(path) => guarded(path, delete::delete_path),
        Action::MoveToTrash(path) => guarded(path, |path| trash::move_to_trash(tools, path)),
        Action::EmptyTrash => trash::empty_trash(tools),
        Action::RunCommand { program, args } => command::run_command(program, args),
        Action::RemoveWorktree {
            repo,
            worktree,
            removal,
        } => worktree::remove_worktree(tools, repo, worktree, *removal),
        Action::PruneWorktrees { repo } => worktree::prune_worktrees(tools, repo),
    }
}

fn record_outcome(item: &Item, outcome: &Outcome) {
    let Some(home) = platform::home_dir() else {
        return;
    };
    log::record(&home, &format!("{} -> {outcome}", item.action));
}

fn guarded(path: &Path, run: impl FnOnce(&Path) -> Outcome) -> Outcome {
    if !is_safe_delete_target(path, platform::home_dir().as_deref()) {
        return Outcome::Failed(format!("refusing to remove {}", path.display()));
    }
    run(path)
}

fn io_outcome(error: &io::Error) -> Outcome {
    match error.kind() {
        io::ErrorKind::PermissionDenied => Outcome::NeedsFullDiskAccess,
        io::ErrorKind::NotFound => Outcome::Skipped("already gone".to_owned()),
        _ => Outcome::Failed(error.to_string()),
    }
}
