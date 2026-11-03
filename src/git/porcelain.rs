use std::path::PathBuf;

pub(crate) const BRANCH_REF_PREFIX: &str = "refs/heads/";
const WORKTREE_KEY: &str = "worktree";

#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[allow(clippy::struct_excessive_bools)]
pub struct WorktreeEntry {
    pub path: PathBuf,
    pub head: Option<String>,
    pub branch: Option<String>,
    pub detached: bool,
    pub bare: bool,
    pub locked: bool,
    pub prunable: bool,
}

fn apply_line(entry: &mut WorktreeEntry, line: &str) {
    let (key, value) = line.split_once(' ').unwrap_or((line, ""));
    match key {
        "worktree" if !value.starts_with('"') => entry.path = PathBuf::from(value),
        "HEAD" => entry.head = Some(value.to_owned()),
        "branch" => {
            let name = value.strip_prefix(BRANCH_REF_PREFIX).unwrap_or(value);
            entry.branch = Some(name.to_owned());
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

fn is_worktree_line(line: &str) -> bool {
    line.split(' ').next() == Some(WORKTREE_KEY)
}

pub fn listed_paths(text: &str) -> Option<Vec<PathBuf>> {
    let announced = text.lines().filter(|line| is_worktree_line(line)).count();
    let paths: Vec<PathBuf> = parse_porcelain(text)
        .into_iter()
        .map(|entry| entry.path)
        .collect();
    (paths.len() == announced).then_some(paths)
}
