use crate::fsutil::{canonical_or_self, child_dirs};
use crate::scan::support::GIT_DIR;
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

const WALK_SKIPPED_DIRS: &[&str] = &[
    "node_modules",
    "target",
    GIT_DIR,
    ".venv",
    "__pycache__",
    ".worktrees",
];
const LINKED_RECORDS_DIR: &str = "worktrees";
const CLAUDE_WORKTREES: &str = ".claude/worktrees";
const REPO_SCAN_MAX_DEPTH: usize = 8;

fn is_agent_worktree_dir(path: &Path) -> bool {
    path.ends_with(CLAUDE_WORKTREES)
}

fn should_descend(child: &Path) -> bool {
    let name = child.file_name().and_then(|name| name.to_str());
    let skipped = name.is_some_and(|name| WALK_SKIPPED_DIRS.contains(&name));
    !skipped && !is_agent_worktree_dir(child) && !child.join(GIT_DIR).is_file()
}

fn walk_repos(dir: &Path, depth: usize, exclude: &[PathBuf], found: &mut BTreeSet<PathBuf>) {
    if depth > REPO_SCAN_MAX_DEPTH || exclude.iter().any(|path| dir.starts_with(path)) {
        return;
    }
    if dir.join(GIT_DIR).is_dir() {
        found.insert(canonical_or_self(dir));
    }
    for child in child_dirs(dir).iter().filter(|child| should_descend(child)) {
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

pub(super) fn has_linked_worktree_records(repo: &Path) -> bool {
    fs::read_dir(repo.join(GIT_DIR).join(LINKED_RECORDS_DIR))
        .is_ok_and(|mut records| records.next().is_some())
}
