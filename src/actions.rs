use crate::fsutil::remove_any;
use crate::git;
use crate::git::status::local_changes;
use crate::model::{Action, Item, Outcome, Removal, RunMode};
use crate::platform::{self, IS_MACOS, NO_HOME, volume_is_mounted};
use crate::process::run_shielded;
use crate::safety::is_safe_delete_target;
use crate::time;
use crate::tools::{self, Tools};
use crate::{freedesktop_trash, log};
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const EMPTY_TRASH_SCRIPT: &str = "tell application \"Finder\" to empty trash";
const STDERR_TAIL_CHARS: usize = 300;
const PERMISSION_MARKERS: [&str; 2] = ["Operation not permitted", "(-54)"];
const AUTOMATION_MARKER: &str = "(-1743)";
const AUTOMATION_BLOCKED: &str = "macOS blocked controlling Finder. Allow your terminal in System Settings > Privacy & Security > Automation";
const CHANGED_SINCE_SCAN: &str = "it changed since the scan, run diskdiet again";
const PRUNE_LINE_PREFIX: &str = "Removing worktrees/";
const WORKTREE_ADMIN_DIR: &str = ".git/worktrees";
const GITDIR_FILE: &str = "gitdir";
const UNPLUGGED_WORKTREE: &str =
    "a worktree of this repo is on a drive that is not plugged in, plug it in first";
const OTHER_DRIVE: &str =
    "it is on another drive, so it cannot go to your Trash. Delete it by hand";

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
        Action::DeletePath(path) => guarded(path, delete_path),
        Action::MoveToTrash(path) => guarded(path, |path| move_to_trash(tools, path)),
        Action::EmptyTrash => empty_trash(tools),
        Action::RunCommand { program, args } => run_command(program, args),
        Action::RemoveWorktree {
            repo,
            worktree,
            removal,
        } => remove_worktree(tools, repo, worktree, *removal),
        Action::PruneWorktrees { repo } => prune_worktrees(tools, repo),
    }
}

pub fn applescript_escape(text: &str) -> String {
    text.replace('\\', "\\\\").replace('"', "\\\"")
}

pub fn trash_script(path: &Path) -> String {
    let escaped = applescript_escape(&path.to_string_lossy());
    format!("tell application \"Finder\" to delete POSIX file \"{escaped}\"")
}

fn record_outcome(item: &Item, outcome: &Outcome) {
    let Some(home) = platform::home_dir() else {
        return;
    };
    log::record(&home, &format!("{} -> {outcome}", item.action));
}

fn io_outcome(error: &io::Error) -> Outcome {
    match error.kind() {
        io::ErrorKind::PermissionDenied => Outcome::NeedsFullDiskAccess,
        io::ErrorKind::NotFound => Outcome::Skipped("already gone".to_owned()),
        _ => Outcome::Failed(error.to_string()),
    }
}

fn guarded(path: &Path, run: impl FnOnce(&Path) -> Outcome) -> Outcome {
    if !is_safe_delete_target(path, platform::home_dir().as_deref()) {
        return Outcome::Failed(format!("refusing to remove {}", path.display()));
    }
    run(path)
}

fn trash_outcome(result: io::Result<()>) -> Outcome {
    match result {
        Ok(()) => Outcome::Done,
        Err(error) if error.kind() == io::ErrorKind::CrossesDevices => {
            Outcome::Failed(OTHER_DRIVE.to_owned())
        }
        Err(error) => io_outcome(&error),
    }
}

fn move_to_trash(tools: &Tools, path: &Path) -> Outcome {
    if IS_MACOS {
        return run_osascript(tools, &trash_script(path));
    }
    let Some(home) = platform::home_dir() else {
        return Outcome::Failed(NO_HOME.to_owned());
    };
    let deleted_at = time::format_timestamp(time::now_secs()).replace(' ', "T");
    trash_outcome(freedesktop_trash::move_to(
        &platform::trash_dir(&home),
        path,
        &deleted_at,
    ))
}

fn empty_trash(tools: &Tools) -> Outcome {
    if IS_MACOS {
        return run_osascript(tools, EMPTY_TRASH_SCRIPT);
    }
    let Some(home) = platform::home_dir() else {
        return Outcome::Failed(NO_HOME.to_owned());
    };
    trash_outcome(freedesktop_trash::empty(&platform::trash_dir(&home)))
}

fn delete_path(path: &Path) -> Outcome {
    remove_any(path).map_or_else(|error| io_outcome(&error), |()| Outcome::Done)
}

fn last_chars(text: &str) -> String {
    let trimmed = text.trim();
    let count = trimmed.chars().count();
    if count <= STDERR_TAIL_CHARS {
        return trimmed.to_owned();
    }
    trimmed.chars().skip(count - STDERR_TAIL_CHARS).collect()
}

fn needs_permission(stderr: &str) -> bool {
    PERMISSION_MARKERS
        .iter()
        .any(|marker| stderr.contains(marker))
}

fn output_outcome(output: &Output) -> Outcome {
    if output.status.success() {
        return Outcome::Done;
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    if stderr.contains(AUTOMATION_MARKER) {
        return Outcome::Failed(AUTOMATION_BLOCKED.to_owned());
    }
    if needs_permission(&stderr) {
        return Outcome::NeedsFullDiskAccess;
    }
    let detail = last_chars(&stderr);
    if detail.is_empty() {
        return Outcome::Failed(format!("exited with {}", output.status));
    }
    Outcome::Failed(detail)
}

fn capture_shielded_from_ctrl_c(command: &mut Command) -> Outcome {
    match run_shielded(command) {
        Ok(output) => output_outcome(&output),
        Err(error) => Outcome::Failed(error.to_string()),
    }
}

fn run_command(program: &Path, args: &[String]) -> Outcome {
    capture_shielded_from_ctrl_c(Command::new(program).args(args))
}

fn run_osascript(tools: &Tools, script: &str) -> Outcome {
    let program = tools.get_or(tools::OSASCRIPT, tools::OSASCRIPT_FALLBACK);
    capture_shielded_from_ctrl_c(Command::new(program).arg("-e").arg(script))
}

fn git_command(tools: &Tools, repo: &Path) -> Command {
    let mut command = Command::new(git::program(tools));
    command.arg("-C").arg(repo);
    command
}

fn changed_since_scan(tools: &Tools, worktree: &Path) -> bool {
    local_changes(git::program(tools), worktree)
        .is_none_or(|counts| counts.dirty > 0 || counts.risky_ignored > 0)
}

fn remove_worktree(tools: &Tools, repo: &Path, worktree: &Path, removal: Removal) -> Outcome {
    if removal == Removal::Rechecked && changed_since_scan(tools, worktree) {
        return Outcome::Skipped(CHANGED_SINCE_SCAN.to_owned());
    }
    let mut remove = git_command(tools, repo);
    remove.args(["worktree", "remove"]);
    if removal == Removal::Forced {
        remove.arg("--force");
    }
    capture_shielded_from_ctrl_c(remove.arg("--").arg(worktree))
}

pub fn prune_names(dry_run_output: &str) -> Vec<String> {
    dry_run_output
        .lines()
        .filter_map(|line| line.strip_prefix(PRUNE_LINE_PREFIX))
        .filter_map(|rest| rest.split_once(':').map(|(name, _)| name.to_owned()))
        .collect()
}

fn recorded_location(repo: &Path, name: &str) -> Option<PathBuf> {
    let gitdir = repo.join(WORKTREE_ADMIN_DIR).join(name).join(GITDIR_FILE);
    let text = std::fs::read_to_string(gitdir).ok()?;
    Some(PathBuf::from(text.trim()))
}

fn prune_would_forget_unplugged(tools: &Tools, repo: &Path) -> bool {
    let mut preview = git_command(tools, repo);
    preview.args(["worktree", "prune", "--dry-run", "--verbose"]);
    let Ok(output) = preview.output() else {
        return true;
    };
    prune_names(&String::from_utf8_lossy(&output.stderr))
        .iter()
        .filter_map(|name| recorded_location(repo, name))
        .any(|location| !volume_is_mounted(&location))
}

fn prune_worktrees(tools: &Tools, repo: &Path) -> Outcome {
    if prune_would_forget_unplugged(tools, repo) {
        return Outcome::Skipped(UNPLUGGED_WORKTREE.to_owned());
    }
    capture_shielded_from_ctrl_c(git_command(tools, repo).args(["worktree", "prune"]))
}
