use crate::log;
use crate::model::{Action, Item, Outcome};
use crate::scan::worktrees::{local_changes, volume_is_mounted};
use crate::tools::Tools;
use std::io;
use std::os::unix::process::CommandExt;
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Output};

const FALLBACK_OSASCRIPT: &str = "/usr/bin/osascript";
const FALLBACK_GIT: &str = "git";
const EMPTY_TRASH_SCRIPT: &str = "tell application \"Finder\" to empty trash";
const MIN_DELETE_COMPONENTS: usize = 3;
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
const SHARED_PARENTS: [&str; 2] = ["/users", "/volumes"];

pub fn execute(item: &Item, tools: &Tools, dry_run: bool) -> Outcome {
    if dry_run {
        return Outcome::DryRun;
    }
    let outcome = perform(&item.action, tools);
    record_outcome(item, &outcome);
    outcome
}

pub fn perform(action: &Action, tools: &Tools) -> Outcome {
    match action {
        Action::DeletePath(path) => guarded(path, delete_path),
        Action::MoveToTrash(path) => {
            guarded(path, |path| run_osascript(tools, &trash_script(path)))
        }
        Action::EmptyTrash => run_osascript(tools, EMPTY_TRASH_SCRIPT),
        Action::RunCommand { program, args } => run_command(program, args),
        Action::RemoveWorktree {
            repo,
            worktree,
            force,
        } => remove_worktree(tools, repo, worktree, *force),
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

pub fn describe(action: &Action) -> String {
    match action {
        Action::DeletePath(path) => format!("delete {}", path.display()),
        Action::MoveToTrash(path) => format!("trash {}", path.display()),
        Action::EmptyTrash => "empty trash".to_string(),
        Action::RunCommand { program, args } => {
            format!("run {} {}", program.display(), args.join(" "))
        }
        Action::RemoveWorktree { repo, worktree, .. } => {
            format!(
                "remove worktree {} (repo {})",
                worktree.display(),
                repo.display()
            )
        }
        Action::PruneWorktrees { repo } => format!("prune worktrees (repo {})", repo.display()),
    }
}

fn describe_outcome(outcome: &Outcome) -> String {
    match outcome {
        Outcome::Done => "done".to_string(),
        Outcome::DryRun => "dry-run".to_string(),
        Outcome::Skipped(why) => format!("skipped: {why}"),
        Outcome::Failed(why) => format!("failed: {why}"),
        Outcome::NeedsFullDiskAccess => "needs full disk access".to_string(),
    }
}

fn home() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}

fn record_outcome(item: &Item, outcome: &Outcome) {
    let Some(home) = home() else {
        return;
    };
    let line = format!(
        "{} -> {}",
        describe(&item.action),
        describe_outcome(outcome)
    );
    log::record(&home, &line);
}

fn io_outcome(error: &io::Error) -> Outcome {
    match error.kind() {
        io::ErrorKind::PermissionDenied => Outcome::NeedsFullDiskAccess,
        io::ErrorKind::NotFound => Outcome::Skipped("already gone".to_string()),
        _ => Outcome::Failed(error.to_string()),
    }
}

fn folded(path: &Path) -> PathBuf {
    PathBuf::from(path.to_string_lossy().to_lowercase())
}

fn is_plain_absolute(path: &Path) -> bool {
    path.is_absolute()
        && path.components().count() >= MIN_DELETE_COMPONENTS
        && path
            .components()
            .all(|part| matches!(part, Component::RootDir | Component::Normal(_)))
}

fn guards_home(path: &Path, home: &Path) -> bool {
    let (path, home) = (folded(path), folded(home));
    home.starts_with(&path) || path.parent() == Some(home.as_path())
}

fn is_shared_parent_child(path: &Path) -> bool {
    let parent = path.parent().map(folded);
    SHARED_PARENTS
        .iter()
        .any(|shared| parent.as_deref() == Some(Path::new(shared)))
}

pub fn is_safe_delete_target(path: &Path, home: Option<&Path>) -> bool {
    is_plain_absolute(path)
        && !is_shared_parent_child(path)
        && home.is_none_or(|home| !guards_home(path, home))
}

fn guarded(path: &Path, run: impl FnOnce(&Path) -> Outcome) -> Outcome {
    if !is_safe_delete_target(path, home().as_deref()) {
        return Outcome::Failed(format!("refusing to remove {}", path.display()));
    }
    run(path)
}

fn delete_path(path: &Path) -> Outcome {
    let metadata = match std::fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) => return io_outcome(&error),
    };
    let result = if metadata.is_dir() {
        std::fs::remove_dir_all(path)
    } else {
        std::fs::remove_file(path)
    };
    result.map_or_else(|error| io_outcome(&error), |()| Outcome::Done)
}

fn tail(text: &str) -> String {
    let trimmed = text.trim();
    let count = trimmed.chars().count();
    if count <= STDERR_TAIL_CHARS {
        return trimmed.to_string();
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
        return Outcome::Failed(AUTOMATION_BLOCKED.to_string());
    }
    if needs_permission(&stderr) {
        return Outcome::NeedsFullDiskAccess;
    }
    let detail = tail(&stderr);
    if detail.is_empty() {
        return Outcome::Failed(format!("exited with {}", output.status));
    }
    Outcome::Failed(detail)
}

// A separate process group keeps a Ctrl-C at the terminal from killing the step in progress.
fn capture(command: &mut Command) -> Outcome {
    match command.process_group(0).output() {
        Ok(output) => output_outcome(&output),
        Err(error) => Outcome::Failed(error.to_string()),
    }
}

fn run_command(program: &Path, args: &[String]) -> Outcome {
    capture(Command::new(program).args(args))
}

fn run_osascript(tools: &Tools, script: &str) -> Outcome {
    let program = tools
        .get("osascript")
        .unwrap_or(Path::new(FALLBACK_OSASCRIPT));
    capture(Command::new(program).arg("-e").arg(script))
}

fn git_command(tools: &Tools, repo: &Path) -> Command {
    let mut command = Command::new(git_path(tools));
    command.arg("-C").arg(repo);
    command
}

fn git_path(tools: &Tools) -> &Path {
    tools.get("git").unwrap_or(Path::new(FALLBACK_GIT))
}

fn changed_since_scan(tools: &Tools, worktree: &Path) -> bool {
    local_changes(git_path(tools), worktree)
        .is_none_or(|counts| counts.dirty > 0 || counts.risky_ignored > 0)
}

fn remove_worktree(tools: &Tools, repo: &Path, worktree: &Path, force: bool) -> Outcome {
    if !force && changed_since_scan(tools, worktree) {
        return Outcome::Skipped(CHANGED_SINCE_SCAN.to_string());
    }
    let mut remove = git_command(tools, repo);
    remove.args(["worktree", "remove"]);
    if force {
        remove.arg("--force");
    }
    capture(remove.arg("--").arg(worktree))
}

pub fn prune_names(dry_run_output: &str) -> Vec<String> {
    dry_run_output
        .lines()
        .filter_map(|line| line.strip_prefix(PRUNE_LINE_PREFIX))
        .filter_map(|rest| rest.split_once(':').map(|(name, _)| name.to_string()))
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
        return Outcome::Skipped(UNPLUGGED_WORKTREE.to_string());
    }
    capture(git_command(tools, repo).args(["worktree", "prune"]))
}
