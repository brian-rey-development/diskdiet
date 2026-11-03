use crate::git::porcelain::BRANCH_REF_PREFIX;
use crate::git::run as run_git;
use crate::git::status::local_changes;
use crate::model::Risk;
use crate::time::secs_since;
use std::path::Path;
use std::time::{Duration, UNIX_EPOCH};

const BASE_CANDIDATES: &[&str] = &["main", "master", "development", "develop"];
const SUBMODULES_FILE: &str = ".gitmodules";
const ASSUMED_AHEAD_WHEN_BASE_UNKNOWN: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Assessment {
    pub ahead: Option<u32>,
    pub dirty: u32,
    pub risky_ignored: u32,
    pub has_submodules: bool,
    pub last_commit_secs: Option<u64>,
}

impl Assessment {
    pub fn risk(&self) -> Risk {
        let ahead = self.ahead.unwrap_or(ASSUMED_AHEAD_WHEN_BASE_UNKNOWN);
        if self.has_submodules {
            return Risk::Danger;
        }
        risk_from_counts(ahead, self.dirty, self.risky_ignored)
    }
}

pub fn risk_from_counts(ahead: u32, dirty: u32, risky_ignored: u32) -> Risk {
    if dirty > 0 || risky_ignored > 0 {
        return Risk::Danger;
    }
    if ahead > 0 { Risk::Review } else { Risk::Safe }
}

fn ref_exists(git: &Path, repo: &Path, name: &str) -> bool {
    let spec = format!("{name}^{{commit}}");
    run_git(git, repo, &["rev-parse", "--verify", "--quiet", &spec]).is_some()
}

fn remote_default_branch(git: &Path, repo: &Path) -> Option<String> {
    let head = run_git(
        git,
        repo,
        &["symbolic-ref", "--short", "refs/remotes/origin/HEAD"],
    )?;
    let name = head.trim();
    (!name.is_empty() && ref_exists(git, repo, name)).then(|| name.to_owned())
}

fn first_local_candidate(git: &Path, repo: &Path) -> Option<String> {
    BASE_CANDIDATES
        .iter()
        .map(|name| format!("{BRANCH_REF_PREFIX}{name}"))
        .find(|full| ref_exists(git, repo, full))
}

pub fn base_branch(git: &Path, repo: &Path) -> Option<String> {
    remote_default_branch(git, repo).or_else(|| first_local_candidate(git, repo))
}

fn count_ahead(git: &Path, worktree: &Path, base: &str) -> Option<u32> {
    let range = format!("{base}..HEAD");
    let out = run_git(git, worktree, &["rev-list", "--count", &range])?;
    out.trim().parse().ok()
}

fn last_commit_age_secs(git: &Path, worktree: &Path) -> Option<u64> {
    let out = run_git(git, worktree, &["log", "-1", "--format=%ct"])?;
    let committed = UNIX_EPOCH + Duration::from_secs(out.trim().parse().ok()?);
    Some(secs_since(committed))
}

pub fn assess_worktree(git: &Path, worktree: &Path, base: Option<&str>) -> Option<Assessment> {
    let counts = local_changes(git, worktree)?;
    Some(Assessment {
        ahead: base.and_then(|name| count_ahead(git, worktree, name)),
        dirty: counts.dirty,
        risky_ignored: counts.risky_ignored,
        has_submodules: worktree.join(SUBMODULES_FILE).is_file(),
        last_commit_secs: last_commit_age_secs(git, worktree),
    })
}
