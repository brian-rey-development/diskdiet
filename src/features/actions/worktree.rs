use super::command::capture_shielded_from_ctrl_c;
use crate::domain::model::{Outcome, Removal};
use crate::infra::git;
use crate::infra::git::porcelain::listed_paths;
use crate::infra::git::status::local_changes;
use crate::infra::sys::platform::volume_is_mounted;
use crate::infra::sys::tools::Tools;
use std::path::Path;

const CHANGED_SINCE_SCAN: &str = "it changed since the scan, run diskdiet again";
const LIST_WORKTREES: [&str; 3] = ["worktree", "list", "--porcelain"];
const UNPLUGGED_WORKTREE: &str =
    "a worktree of this repo is on a drive that is not plugged in, plug it in first";
const LISTING_FAILED: &str =
    "git could not list the worktrees of this repo, so nothing was forgotten";

pub(super) fn remove_worktree(
    tools: &Tools,
    repo: &Path,
    worktree: &Path,
    removal: Removal,
) -> Outcome {
    if removal == Removal::Rechecked && changed_since_scan(tools, worktree) {
        return Outcome::Skipped(CHANGED_SINCE_SCAN.to_owned());
    }
    let mut remove = git::command(git::program(tools), repo, &["worktree", "remove"]);
    if removal == Removal::Forced {
        remove.arg("--force");
    }
    capture_shielded_from_ctrl_c(remove.arg("--").arg(worktree))
}

pub(super) fn prune_worktrees(tools: &Tools, repo: &Path) -> Outcome {
    if let Some(reason) = prune_refusal(tools, repo) {
        return Outcome::Skipped(reason.to_owned());
    }
    let mut prune = git::command(git::program(tools), repo, &["worktree", "prune"]);
    capture_shielded_from_ctrl_c(&mut prune)
}

fn changed_since_scan(tools: &Tools, worktree: &Path) -> bool {
    local_changes(git::program(tools), worktree)
        .is_none_or(|counts| counts.dirty > 0 || counts.risky_ignored > 0)
}

fn prune_refusal(tools: &Tools, repo: &Path) -> Option<&'static str> {
    let Some(listing) = git::run(git::program(tools), repo, &LIST_WORKTREES) else {
        return Some(LISTING_FAILED);
    };
    let unplugged = listed_paths(&listing)
        .is_none_or(|paths| paths.iter().any(|path| !volume_is_mounted(path)));
    unplugged.then_some(UNPLUGGED_WORKTREE)
}
