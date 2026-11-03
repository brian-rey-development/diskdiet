use super::ScanContext;
use super::builds::{format_age, plural_s, secs_since, tilde_path};
use crate::model::{Action, Category, Item, Risk};
use crate::size::{disk_usage, format_bytes};
use crate::tools::run_with_deadline;
use rayon::prelude::*;
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, UNIX_EPOCH};

const BUILD_OUTPUT_DIRS: [&str; 17] = [
    "target",
    "node_modules",
    ".next",
    ".turbo",
    "dist",
    "build",
    ".venv",
    "__pycache__",
    "coverage",
    ".cache",
    ".parcel-cache",
    ".pytest_cache",
    ".ruff_cache",
    ".mypy_cache",
    ".svelte-kit",
    ".nuxt",
    ".tanstack",
];
const IGNORABLE_FILES: [&str; 3] = [".DS_Store", "next-env.d.ts", ".eslintcache"];
const IGNORABLE_EXTENSIONS: [&str; 2] = [".tsbuildinfo", ".pyc"];
const GENERATED_DIR_PAIRS: [[&str; 2]; 2] = [[".husky", "_"], ["src-tauri", "gen"]];
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
const STATUS_ARGS: [&str; 6] = [
    "status",
    "--porcelain=v1",
    "-z",
    "--ignored=matching",
    "--untracked-files=normal",
    "--ignore-submodules=none",
];
const LS_FILES_ARGS: [&str; 3] = ["ls-files", "-v", "-z"];
const SKIP_WORKTREE_TAG: &str = "S";
const SUBMODULES_FILE: &str = ".gitmodules";
const GIT_TIMEOUT: Duration = Duration::from_secs(30);
const NO_FSMONITOR: [&str; 2] = ["-c", "core.fsmonitor=false"];
const IGNORED_CODE: &str = "!!";
const STATUS_CODE_LEN: usize = 2;
const STATUS_PATH_OFFSET: usize = 3;
const MAX_WALK_DEPTH: usize = 8;
const UNKNOWN_AHEAD: u32 = 1;
const DELETE_WORD: &str = "delete";
const VOLUMES_DIR: &str = "/Volumes";
const BRANCH_REF_PREFIX: &str = "refs/heads/";

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct WorktreeEntry {
    pub path: PathBuf,
    pub head: Option<String>,
    pub branch: Option<String>,
    pub detached: bool,
    pub bare: bool,
    pub locked: bool,
    pub prunable: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct StatusCounts {
    pub dirty: u32,
    pub risky_ignored: u32,
}

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

fn is_generated_file(name: &str) -> bool {
    IGNORABLE_FILES.contains(&name) || IGNORABLE_EXTENSIONS.iter().any(|ext| name.ends_with(ext))
}

pub fn is_build_output(path: &str) -> bool {
    let parts: Vec<&str> = path.split('/').filter(|part| !part.is_empty()).collect();
    let Some(last) = parts.last() else {
        return false;
    };
    let in_generated_dir = parts
        .windows(2)
        .any(|pair| GENERATED_DIR_PAIRS.iter().any(|known| known == pair));
    if in_generated_dir {
        return true;
    }
    if !path.ends_with('/') {
        return is_generated_file(last);
    }
    BUILD_OUTPUT_DIRS.contains(last)
}

fn apply_line(entry: &mut WorktreeEntry, line: &str) {
    let (key, value) = line.split_once(' ').unwrap_or((line, ""));
    match key {
        "worktree" if !value.starts_with('"') => entry.path = PathBuf::from(value),
        "HEAD" => entry.head = Some(value.to_string()),
        "branch" => {
            let name = value.strip_prefix(BRANCH_REF_PREFIX).unwrap_or(value);
            entry.branch = Some(name.to_string());
        }
        "detached" => entry.detached = true,
        "bare" => entry.bare = true,
        "locked" => entry.locked = true,
        "prunable" => entry.prunable = true,
        _ => {}
    }
}

fn parse_block(block: &str) -> Option<WorktreeEntry> {
    let mut entry = WorktreeEntry::default();
    for line in block.lines() {
        apply_line(&mut entry, line);
    }
    (!entry.path.as_os_str().is_empty()).then_some(entry)
}

pub fn parse_porcelain(text: &str) -> Vec<WorktreeEntry> {
    text.split("\n\n").filter_map(parse_block).collect()
}

pub fn count_status(text: &str) -> StatusCounts {
    let mut counts = StatusCounts::default();
    let mut fields = text.split('\0').filter(|field| !field.is_empty());
    while let Some(field) = fields.next() {
        let (Some(code), Some(path)) = (
            field.get(..STATUS_CODE_LEN),
            field.get(STATUS_PATH_OFFSET..),
        ) else {
            counts.dirty += 1;
            continue;
        };
        if code == IGNORED_CODE {
            counts.risky_ignored += u32::from(!is_build_output(path));
            continue;
        }
        counts.dirty += 1;
        if code.contains(['R', 'C']) {
            fields.next();
        }
    }
    counts
}

fn git_command(git: &Path, dir: &Path, args: &[&str]) -> Command {
    let mut command = Command::new(git);
    command
        .arg("-C")
        .arg(dir)
        .args(NO_FSMONITOR)
        .args(args)
        .env("GIT_OPTIONAL_LOCKS", "0")
        .env("GIT_TERMINAL_PROMPT", "0");
    command
}

fn run_git(git: &Path, dir: &Path, args: &[&str]) -> Option<String> {
    let (status, stdout) = run_with_deadline(&mut git_command(git, dir, args), GIT_TIMEOUT)?;
    status
        .success()
        .then(|| String::from_utf8_lossy(&stdout).into_owned())
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

fn is_hidden_edit(field: &str, worktree: &Path) -> bool {
    let Some((tag, path)) = field.split_once(' ') else {
        return false;
    };
    let assume_unchanged = !tag.is_empty() && tag.chars().all(|c| c.is_ascii_lowercase());
    (tag == SKIP_WORKTREE_TAG || assume_unchanged) && worktree.join(path).exists()
}

pub fn count_hidden_edits(listing: &str, worktree: &Path) -> u32 {
    let hidden = listing
        .split('\0')
        .filter(|field| is_hidden_edit(field, worktree))
        .count();
    u32::try_from(hidden).unwrap_or(u32::MAX)
}

pub fn local_changes(git: &Path, worktree: &Path) -> Option<StatusCounts> {
    let status = run_git(git, worktree, &STATUS_ARGS)?;
    let listing = run_git(git, worktree, &LS_FILES_ARGS)?;
    let counts = count_status(&status);
    Some(StatusCounts {
        dirty: counts.dirty + count_hidden_edits(&listing, worktree),
        risky_ignored: counts.risky_ignored,
    })
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

fn canonical(path: &Path) -> PathBuf {
    fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

fn is_agent_worktree_dir(path: &Path) -> bool {
    path.ends_with(CLAUDE_WORKTREES)
}

fn should_descend(child: &Path) -> bool {
    let name = child.file_name().and_then(|name| name.to_str());
    let skipped = name.is_some_and(|name| WALK_SKIPPED_DIRS.contains(&name));
    !skipped && !is_agent_worktree_dir(child) && !child.join(".git").is_file()
}

fn child_dirs(dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    entries
        .flatten()
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_dir()))
        .map(|entry| entry.path())
        .collect()
}

fn walk_repos(dir: &Path, depth: usize, exclude: &[PathBuf], found: &mut BTreeSet<PathBuf>) {
    if depth > MAX_WALK_DEPTH || exclude.iter().any(|path| dir.starts_with(path)) {
        return;
    }
    if dir.join(".git").is_dir() {
        found.insert(canonical(dir));
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

pub fn volume_is_mounted(path: &Path) -> bool {
    let Ok(rest) = path.strip_prefix(VOLUMES_DIR) else {
        return true;
    };
    let volume = rest.components().next();
    volume.is_none_or(|name| Path::new(VOLUMES_DIR).join(name).exists())
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
    let action = Action::RemoveWorktree {
        repo: repo.to_path_buf(),
        worktree: entry.path.clone(),
        force: assessment.risk() == Risk::Danger,
    };
    Some(risky_item(branch, detail, bytes, assessment.risk(), action))
}

fn risky_item(label: &str, detail: String, bytes: u64, risk: Risk, action: Action) -> Item {
    Item {
        category: Category::AgentWorktrees,
        label: label.to_string(),
        detail,
        bytes,
        risk,
        action,
        preselected: risk == Risk::Safe,
        typed_confirmation: (risk == Risk::Danger).then(|| DELETE_WORD.to_string()),
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
    Some(Item {
        category: Category::AgentWorktrees,
        label: format!("Stale worktree records in {name}"),
        detail: prune_detail(missing, repo, home),
        bytes: 0,
        risk: Risk::Safe,
        action: Action::PruneWorktrees {
            repo: repo.to_path_buf(),
        },
        preselected: true,
        typed_confirmation: None,
    })
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
        .filter(|entry| !entry.bare && !entry.locked && canonical(&entry.path) != repo)
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
    let git = ctx
        .tools
        .get("git")
        .map_or_else(|| PathBuf::from("git"), Path::to_path_buf);
    let env = Env {
        git,
        home: ctx.home.clone(),
    };
    find_repos(&ctx.config.dev_roots, &ctx.config.exclude)
        .par_iter()
        .flat_map(|repo| scan_repo(repo, &env))
        .collect()
}
