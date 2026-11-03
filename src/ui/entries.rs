use crate::model::{Action, Category, Item, Risk};
use crate::size::format_bytes;
use console::style;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

const SIZE_COLUMN: usize = 9;
pub(super) const DANGER_TAG: &str = "DANGER";
const REPO_SEPARATOR: &str = "›";

pub type Selection = BTreeSet<usize>;

fn worktree_repo(item: &Item) -> Option<&Path> {
    match &item.action {
        Action::RemoveWorktree { repo, .. } | Action::PruneWorktrees { repo } => Some(repo),
        _ => None,
    }
}

fn repo_name(repo: &Path) -> String {
    repo.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
}

pub fn display_label(item: &Item) -> String {
    match &item.action {
        Action::RemoveWorktree { repo, .. } => {
            format!("{} {REPO_SEPARATOR} {}", repo_name(repo), item.label)
        }
        _ => item.label.clone(),
    }
}

pub fn item_label(item: &Item) -> String {
    let size = format!("{:>SIZE_COLUMN$}", format_bytes(item.bytes));
    let label = display_label(item);
    if item.risk != Risk::Danger {
        return format!("{size}  {label}");
    }
    let tag = style(DANGER_TAG).red().bold();
    format!("{size}  {tag} {}", style(label).red())
}

fn sort_key(item: &Item) -> (PathBuf, String) {
    let repo = worktree_repo(item).map(Path::to_path_buf);
    (repo.unwrap_or_default(), item.label.to_lowercase())
}

pub fn category_entries(items: &[Item], category: Category) -> Vec<(usize, &Item)> {
    let mut entries: Vec<(usize, &Item)> = items
        .iter()
        .enumerate()
        .filter(|(_, item)| item.category == category)
        .collect();
    if category == Category::AgentWorktrees {
        entries.sort_by_cached_key(|(_, item)| sort_key(item));
    }
    entries
}
