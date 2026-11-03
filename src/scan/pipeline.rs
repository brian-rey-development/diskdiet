use crate::fsutil::lowercased;
use crate::model::{Action, DELETE_CONFIRMATION, Item, Risk};
use std::cmp::Ordering;
use std::path::{Path, PathBuf};

const MIN_ITEM_BYTES: u64 = 1_000_000;

fn removed_tree_path(action: &Action) -> Option<&Path> {
    action.path().filter(|_| action.deletes_path())
}

fn path_excluding_prune(action: &Action) -> Option<&Path> {
    match action {
        Action::PruneWorktrees { .. } => None,
        other => other.path(),
    }
}

fn riskiest_child(items: &[Item], parent: &Path) -> Option<Risk> {
    items
        .iter()
        .filter_map(|item| path_excluding_prune(&item.action).map(|path| (path, item.risk)))
        .filter(|(path, _)| *path != parent && path.starts_with(parent))
        .map(|(_, risk)| risk)
        .max()
}

fn inherited_risk(items: &[Item], item: &Item) -> Option<Risk> {
    removed_tree_path(&item.action)
        .and_then(|path| riskiest_child(items, path))
        .filter(|child_risk| *child_risk > item.risk)
}

fn apply_inherited_risk(item: &mut Item, risk: Risk) {
    item.risk = risk;
    item.preselected = false;
    if risk == Risk::Danger {
        item.typed_confirmation = Some(DELETE_CONFIRMATION.to_owned());
    }
}

pub fn inherit_child_risk(items: &mut [Item]) {
    let inherited: Vec<Option<Risk>> = items
        .iter()
        .map(|item| inherited_risk(items, item))
        .collect();
    for (item, risk) in items.iter_mut().zip(inherited) {
        if let Some(risk) = risk {
            apply_inherited_risk(item, risk);
        }
    }
}

pub fn compare_items(left: &Item, right: &Item) -> Ordering {
    left.category
        .cmp(&right.category)
        .then(left.risk.cmp(&right.risk))
        .then(right.bytes.cmp(&left.bytes))
        .then_with(|| left.label.cmp(&right.label))
}

pub fn is_kept(item: &Item, exclude: &[PathBuf]) -> bool {
    if item.bytes < MIN_ITEM_BYTES && !item.action.ignores_size_floor() {
        return false;
    }
    let Some(path) = item.action.path().map(lowercased) else {
        return true;
    };
    !exclude
        .iter()
        .map(|excluded| lowercased(excluded))
        .any(|excluded| path.starts_with(&excluded) || excluded.starts_with(&path))
}
