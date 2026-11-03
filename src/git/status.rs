use super::ignored::is_build_output;
use super::run;
use std::path::Path;

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
const IGNORED_CODE: &str = "!!";
const STATUS_CODE_LEN: usize = 2;
const STATUS_PATH_OFFSET: usize = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct StatusCounts {
    pub dirty: u32,
    pub risky_ignored: u32,
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

fn is_hidden_edit(field: &str, worktree: &Path) -> bool {
    let Some((tag, path)) = field.split_once(' ') else {
        return false;
    };
    let assume_unchanged = !tag.is_empty() && tag.chars().all(|c| c.is_ascii_lowercase());
    (tag == SKIP_WORKTREE_TAG || assume_unchanged) && worktree.join(path).exists()
}

pub(crate) fn count_hidden_edits(listing: &str, worktree: &Path) -> u32 {
    let hidden = listing
        .split('\0')
        .filter(|field| is_hidden_edit(field, worktree))
        .count();
    u32::try_from(hidden).unwrap_or(u32::MAX)
}

pub(crate) fn local_changes(git: &Path, worktree: &Path) -> Option<StatusCounts> {
    let status = run(git, worktree, &STATUS_ARGS)?;
    let listing = run(git, worktree, &LS_FILES_ARGS)?;
    let counts = count_status(&status);
    Some(StatusCounts {
        dirty: counts.dirty + count_hidden_edits(&listing, worktree),
        risky_ignored: counts.risky_ignored,
    })
}
