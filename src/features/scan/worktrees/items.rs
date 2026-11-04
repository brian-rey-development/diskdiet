use super::RepoScanEnv;
use super::assess::{Assessment, assess_worktree};
use crate::domain::model::{Action, Category, Item, Removal, Risk};
use crate::features::scan::support::{DETAIL_SEPARATOR, file_label};
use crate::infra::fs::size::{disk_usage, format_bytes};
use crate::infra::git::porcelain::WorktreeEntry;
use crate::infra::util::fmt::{format_age, plural_s, tilde_path};
use std::path::Path;

#[derive(Clone, Copy)]
struct Worktree<'a> {
    repo: &'a Path,
    path: &'a Path,
}

fn ahead_text(ahead: Option<u32>) -> String {
    match ahead {
        Some(0) => "merged · 0 ahead".to_owned(),
        Some(count) => format!("{count} commit{} ahead", plural_s(u64::from(count))),
        None => "base branch unknown".to_owned(),
    }
}

fn pluralized_count(count: u32, noun: &str, suffix: &str) -> Option<String> {
    (count > 0).then(|| format!("{count} {noun}{}{suffix}", plural_s(u64::from(count))))
}

fn warnings(assessment: &Assessment) -> impl Iterator<Item = String> {
    let submodules = assessment
        .has_submodules
        .then(|| "has submodules".to_owned());
    [
        pluralized_count(assessment.dirty, "uncommitted file", ""),
        pluralized_count(assessment.risky_ignored, "ignored file", " to keep"),
        submodules,
    ]
    .into_iter()
    .flatten()
}

fn describe(assessment: &Assessment, bytes: u64, path: &Path, home: &Path) -> String {
    let mut parts = vec![ahead_text(assessment.ahead)];
    parts.extend(warnings(assessment));
    parts.push(format_bytes(bytes));
    if let Some(secs) = assessment.last_commit_secs {
        parts.push(format!("last commit {}", format_age(secs)));
    }
    parts.push(tilde_path(path, home));
    parts.join(DETAIL_SEPARATOR)
}

fn removal_item(
    label: &str,
    detail: String,
    bytes: u64,
    assessment: &Assessment,
    worktree: Worktree<'_>,
) -> Item {
    let risk = assessment.risk();
    let removal = match risk {
        Risk::Danger => Removal::Forced {
            dirty: assessment.dirty,
            risky_ignored: assessment.risky_ignored,
        },
        _ => Removal::Rechecked,
    };
    let action = Action::RemoveWorktree {
        repo: worktree.repo.to_path_buf(),
        worktree: worktree.path.to_path_buf(),
        removal,
    };
    let item = Item::new(Category::AgentWorktrees, label, detail, bytes, risk, action);
    match removal {
        Removal::Forced { .. } => item.with_typed_delete(),
        Removal::Rechecked => item,
    }
}

fn assess_and_measure(
    entry: &WorktreeEntry,
    base: Option<&str>,
    env: &RepoScanEnv,
) -> Option<(Assessment, u64)> {
    let (assessment, bytes) = rayon::join(
        || assess_worktree(&env.git, &entry.path, base),
        || disk_usage(std::slice::from_ref(&entry.path)),
    );
    Some((assessment?, bytes))
}

pub(super) fn worktree_item(
    repo: &Path,
    entry: &WorktreeEntry,
    base: Option<&str>,
    env: &RepoScanEnv,
) -> Option<Item> {
    let branch = entry
        .branch
        .as_deref()
        .filter(|_| !entry.detached && !entry.bare)?;
    let (assessment, bytes) = assess_and_measure(entry, base, env)?;
    let detail = describe(&assessment, bytes, &entry.path, &env.home);
    let worktree = Worktree {
        repo,
        path: &entry.path,
    };
    Some(removal_item(branch, detail, bytes, &assessment, worktree))
}

fn prune_detail(missing: usize, repo: &Path, home: &Path) -> String {
    let count = u64::try_from(missing).unwrap_or(u64::MAX);
    let verb = if count == 1 {
        "no longer exists"
    } else {
        "no longer exist"
    };
    let noun = format!("worktree folder{}", plural_s(count));
    let repo_text = tilde_path(repo, home);
    format!("{count} {noun} {verb}{DETAIL_SEPARATOR}{repo_text}")
}

pub(super) fn prune_item(repo: &Path, missing: usize, home: &Path) -> Option<Item> {
    if missing == 0 {
        return None;
    }
    let name = file_label(repo).unwrap_or_default();
    Some(Item::new(
        Category::AgentWorktrees,
        format!("Stale worktree records in {name}"),
        prune_detail(missing, repo, home),
        0,
        Risk::Safe,
        Action::PruneWorktrees {
            repo: repo.to_path_buf(),
        },
    ))
}
