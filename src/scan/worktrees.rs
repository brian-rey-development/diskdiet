use super::ScanContext;
use crate::fmt::{format_age, plural_s, tilde_path};
use crate::fsutil::{canonical_or_self, child_dirs};
use crate::git;
use crate::git::porcelain::{BRANCH_REF_PREFIX, WorktreeEntry, parse_porcelain};
use crate::git::run as run_git;
use crate::git::status::local_changes;
use crate::model::{Action, Category, Item, Removal, Risk};
use crate::platform::volume_is_mounted;
use crate::size::{disk_usage, format_bytes};
use crate::time::secs_since;
use rayon::prelude::*;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::time::{Duration, UNIX_EPOCH};

const BASE_CANDIDATES: [&str; 4] = ["main", "master", "development", "develop"];
const WALK_SKIPPED_DIRS: [&str; 6] = [
    "node_modules",
    "target",
    ".git",
    ".venv",
    "__pycache__",
    ".worktrees",
];
const CLAUDE_WORKTREES: &str = ".claude/worktrees";
const SUBMODULES_FILE: &str = ".gitmodules";
const MAX_WALK_DEPTH: usize = 8;
const UNKNOWN_AHEAD: u32 = 1;

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
        let ahead = self.ahead.unwrap_or(UNKNOWN_AHEAD);
        if self.has_submodules {
            return Risk::Danger;
        }
        assess(ahead, self.dirty, self.risky_ignored)
    }
}

pub fn assess(ahead: u32, dirty: u32, risky_ignored: u32) -> Risk {
    if dirty > 0 || risky_ignored > 0 {
        return Risk::Danger;
    }
    if ahead > 0 { Risk::Review } else { Risk::Safe }
}

fn ref_exists(git: &Path, repo: &Path, name: &str) -> bool {
    let spec = format!("{name}^{{commit}}");
    run_git(git, repo, &["rev-parse", "--verify", "--quiet", &spec]).is_some()
}

pub fn base_branch(git: &Path, repo: &Path) -> Option<String> {
    let head = run_git(
        git,
        repo,
        &["symbolic-ref", "--short", "refs/remotes/origin/HEAD"],
    );
    let remote_base = head
        .map(|text| text.trim().to_string())
        .filter(|name| !name.is_empty() && ref_exists(git, repo, name));
    remote_base.or_else(|| {
        BASE_CANDIDATES
            .iter()
            .map(|name| format!("{BRANCH_REF_PREFIX}{name}"))
            .find(|full| ref_exists(git, repo, full))
    })
}

fn count_ahead(git: &Path, worktree: &Path, base: &str) -> Option<u32> {
    let range = format!("{base}..HEAD");
    let out = run_git(git, worktree, &["rev-list", "--count", &range])?;
    out.trim().parse().ok()
}

fn last_commit_secs(git: &Path, worktree: &Path) -> Option<u64> {
    let out = run_git(git, worktree, &["log", "-1", "--format=%ct"])?;
    let committed = UNIX_EPOCH + Duration::from_secs(out.trim().parse().ok()?);
    Some(secs_since(committed))
}

pub fn inspect(git: &Path, worktree: &Path, base: Option<&str>) -> Option<Assessment> {
    let counts = local_changes(git, worktree)?;
    Some(Assessment {
        ahead: base.and_then(|name| count_ahead(git, worktree, name)),
        dirty: counts.dirty,
        risky_ignored: counts.risky_ignored,
        has_submodules: worktree.join(SUBMODULES_FILE).is_file(),
        last_commit_secs: last_commit_secs(git, worktree),
    })
}

fn is_agent_worktree_dir(path: &Path) -> bool {
    path.ends_with(CLAUDE_WORKTREES)
}

fn should_descend(child: &Path) -> bool {
    let name = child.file_name().and_then(|name| name.to_str());
    let skipped = name.is_some_and(|name| WALK_SKIPPED_DIRS.contains(&name));
    !skipped && !is_agent_worktree_dir(child) && !child.join(".git").is_file()
}

fn walk_repos(dir: &Path, depth: usize, exclude: &[PathBuf], found: &mut BTreeSet<PathBuf>) {
    if depth > MAX_WALK_DEPTH || exclude.iter().any(|path| dir.starts_with(path)) {
        return;
    }
    if dir.join(".git").is_dir() {
        found.insert(canonical_or_self(dir));
    }
    for child in child_dirs(dir).iter().filter(|c| should_descend(c)) {
        walk_repos(child, depth + 1, exclude, found);
    }
}

pub fn find_repos(roots: &[PathBuf], exclude: &[PathBuf]) -> Vec<PathBuf> {
    let mut found = BTreeSet::new();
    for root in roots {
        walk_repos(root, 0, exclude, &mut found);
    }
    found.into_iter().collect()
}

fn folder_is_missing(entry: &WorktreeEntry) -> bool {
    matches!(entry.path.try_exists(), Ok(false)) && volume_is_mounted(&entry.path)
}

fn ahead_text(ahead: Option<u32>) -> String {
    match ahead {
        Some(0) => "merged · 0 ahead".to_string(),
        Some(count) => format!("{count} commit{} ahead", plural_s(u64::from(count))),
        None => "base branch unknown".to_string(),
    }
}

fn counted(count: u32, noun: &str, suffix: &str) -> Option<String> {
    (count > 0).then(|| format!("{count} {noun}{}{suffix}", plural_s(u64::from(count))))
}

fn warnings(assessment: &Assessment) -> impl Iterator<Item = String> {
    let submodules = assessment
        .has_submodules
        .then(|| "has submodules".to_string());
    [
        counted(assessment.dirty, "uncommitted file", ""),
        counted(assessment.risky_ignored, "ignored file", " to keep"),
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
    parts.join(" · ")
}

fn worktree_item(
    repo: &Path,
    entry: &WorktreeEntry,
    base: Option<&str>,
    ctx: &Env,
) -> Option<Item> {
    let branch = entry
        .branch
        .as_deref()
        .filter(|_| !entry.detached && !entry.bare)?;
    let assessment = inspect(&ctx.git, &entry.path, base)?;
    let bytes = disk_usage(std::slice::from_ref(&entry.path));
    let detail = describe(&assessment, bytes, &entry.path, &ctx.home);
    let risk = assessment.risk();
    let removal = match risk {
        Risk::Danger => Removal::Forced,
        _ => Removal::Rechecked,
    };
    let action = Action::RemoveWorktree {
        repo: repo.to_path_buf(),
        worktree: entry.path.clone(),
        removal,
    };
    Some(risky_item(branch, detail, bytes, risk, action))
}

fn risky_item(label: &str, detail: String, bytes: u64, risk: Risk, action: Action) -> Item {
    let item = Item::new(Category::AgentWorktrees, label, detail, bytes, risk, action);
    match risk {
        Risk::Danger => item.with_typed_delete(),
        _ => item,
    }
}

fn prune_detail(missing: usize, repo: &Path, home: &Path) -> String {
    let count = u64::try_from(missing).unwrap_or(u64::MAX);
    let verb = if count == 1 {
        "no longer exists"
    } else {
        "no longer exist"
    };
    let noun = format!("worktree folder{}", plural_s(count));
    format!("{count} {noun} {verb} · {}", tilde_path(repo, home))
}

fn prune_item(repo: &Path, missing: usize, home: &Path) -> Option<Item> {
    if missing == 0 {
        return None;
    }
    let name = repo
        .file_name()
        .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
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

struct Env {
    git: PathBuf,
    home: PathBuf,
}

fn scan_repo(repo: &Path, ctx: &Env) -> Vec<Item> {
    let Some(listing) = run_git(&ctx.git, repo, &["worktree", "list", "--porcelain"]) else {
        return Vec::new();
    };
    let linked: Vec<WorktreeEntry> = parse_porcelain(&listing)
        .into_iter()
        .filter(|entry| !entry.bare && !entry.locked && canonical_or_self(&entry.path) != repo)
        .collect();
    let unplugged = linked.iter().any(|entry| !volume_is_mounted(&entry.path));
    let (missing, present): (Vec<_>, Vec<_>) = linked.into_iter().partition(folder_is_missing);
    let base = base_branch(&ctx.git, repo);
    let mut items: Vec<Item> = present
        .par_iter()
        .filter_map(|entry| worktree_item(repo, entry, base.as_deref(), ctx))
        .collect();
    if !unplugged {
        items.extend(prune_item(repo, missing.len(), &ctx.home));
    }
    items
}

pub fn scan(ctx: &ScanContext) -> Vec<Item> {
    let env = Env {
        git: git::program(&ctx.tools).to_path_buf(),
        home: ctx.home.clone(),
    };
    find_repos(&ctx.config.dev_roots, &ctx.config.exclude)
        .par_iter()
        .flat_map(|repo| scan_repo(repo, &env))
        .collect()
}
