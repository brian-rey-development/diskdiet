use super::command::capture_shielded_from_ctrl_c;
use crate::git;
use crate::git::status::local_changes;
use crate::model::{Outcome, Removal};
use crate::platform::volume_is_mounted;
use crate::tools::Tools;
use std::path::{Path, PathBuf};
use std::process::Command;

const CHANGED_SINCE_SCAN: &str = "it changed since the scan, run diskdiet again";
const PRUNE_LINE_PREFIX: &str = "Removing worktrees/";
const WORKTREE_ADMIN_DIR: &str = ".git/worktrees";
const GITDIR_FILE: &str = "gitdir";
const UNPLUGGED_WORKTREE: &str =
    "a worktree of this repo is on a drive that is not plugged in, plug it in first";

pub fn prune_names(dry_run_output: &str) -> Vec<String> {
    dry_run_output
        .lines()
        .filter_map(|line| line.strip_prefix(PRUNE_LINE_PREFIX))
        .filter_map(|rest| rest.split_once(':').map(|(name, _)| name.to_owned()))
        .collect()
}

pub(super) fn remove_worktree(
    tools: &Tools,
    repo: &Path,
    worktree: &Path,
    removal: Removal,
) -> Outcome {
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

pub(super) fn prune_worktrees(tools: &Tools, repo: &Path) -> Outcome {
    if prune_would_forget_unplugged(tools, repo) {
        return Outcome::Skipped(UNPLUGGED_WORKTREE.to_owned());
    }
    capture_shielded_from_ctrl_c(git_command(tools, repo).args(["worktree", "prune"]))
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

fn previewed_prune_names(tools: &Tools, repo: &Path) -> Option<Vec<String>> {
    let mut preview = git_command(tools, repo);
    preview.args(["worktree", "prune", "--dry-run", "--verbose"]);
    let output = preview.output().ok()?;
    Some(prune_names(&String::from_utf8_lossy(&output.stderr)))
}

fn recorded_location(repo: &Path, name: &str) -> Option<PathBuf> {
    let gitdir = repo.join(WORKTREE_ADMIN_DIR).join(name).join(GITDIR_FILE);
    let text = std::fs::read_to_string(gitdir).ok()?;
    Some(PathBuf::from(text.trim()))
}

fn prune_would_forget_unplugged(tools: &Tools, repo: &Path) -> bool {
    let Some(names) = previewed_prune_names(tools, repo) else {
        return true;
    };
    names
        .iter()
        .filter_map(|name| recorded_location(repo, name))
        .any(|location| !volume_is_mounted(&location))
}
