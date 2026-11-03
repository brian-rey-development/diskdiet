use crate::support::fixtures::{delete_item, sized_item};
use diskdiet::domain::model::{Action, Category, DELETE_CONFIRMATION, Item, Removal, Risk};
use diskdiet::features::scan::{compare_items, inherit_child_risk, is_kept};
use std::path::{Path, PathBuf};

const BIG: u64 = 5_000_000;
const ONE_MEGABYTE: u64 = 1_000_000;

fn safe_delete(path: &str, bytes: u64) -> Item {
    delete_item(Path::new(path), Risk::Safe, bytes)
}

fn trim_command() -> Action {
    Action::RunCommand {
        program: PathBuf::from("/usr/local/bin/colima"),
        args: ["ssh", "--", "sudo", "fstrim", "-a"]
            .map(String::from)
            .to_vec(),
    }
}

fn sort_key(items: &[Item]) -> Vec<(Category, Risk, u64)> {
    items
        .iter()
        .map(|item| (item.category, item.risk, item.bytes))
        .collect()
}

#[test]
fn is_kept_drops_items_below_one_megabyte() {
    assert!(!is_kept(&safe_delete("/a", 0), &[]));
    assert!(!is_kept(&safe_delete("/a", ONE_MEGABYTE - 1), &[]));
    assert!(is_kept(&safe_delete("/a", ONE_MEGABYTE), &[]));
    let other = Action::RunCommand {
        program: PathBuf::from("/bin/ls"),
        args: vec![],
    };
    let tiny = sized_item(Category::Docker, Risk::Safe, 0, other);
    assert!(!is_kept(&tiny, &[]));
}

#[test]
fn is_kept_ignores_size_floor_for_worktrees_trash_and_fstrim() {
    let prune = Action::PruneWorktrees {
        repo: PathBuf::from("/r"),
    };
    let worktrees = sized_item(Category::AgentWorktrees, Risk::Safe, 0, prune);
    let trash = sized_item(Category::Trash, Risk::Review, 0, Action::EmptyTrash);
    let trim = sized_item(Category::Docker, Risk::Safe, 0, trim_command());
    assert!(is_kept(&worktrees, &[]));
    assert!(is_kept(&trash, &[]));
    assert!(is_kept(&trim, &[]));
}

#[test]
fn is_kept_keeps_tiny_worktrees() {
    let worktree = Action::RemoveWorktree {
        repo: PathBuf::from("/r"),
        worktree: PathBuf::from("/r/wt"),
        removal: Removal::Rechecked,
    };
    let tiny = sized_item(Category::AgentWorktrees, Risk::Danger, 1, worktree);
    assert!(is_kept(&tiny, &[]));
}

#[test]
fn is_kept_honours_exclude_in_both_directions() {
    let exclude = [PathBuf::from("/home/me/keep")];
    assert!(!is_kept(
        &safe_delete("/home/me/keep/proj/target", BIG),
        &exclude
    ));
    assert!(!is_kept(&safe_delete("/home/me/keep", BIG), &exclude));
    assert!(!is_kept(&safe_delete("/home/me", BIG), &exclude));
    assert!(is_kept(
        &safe_delete("/home/me/keeper/target", BIG),
        &exclude
    ));
    assert!(is_kept(&safe_delete("/home/other/target", BIG), &exclude));
}

#[test]
fn is_kept_matches_exclusions_regardless_of_case() {
    let exclude = [PathBuf::from("/home/me/develop/keep")];
    let item = safe_delete("/home/me/Develop/Keep/target", BIG);
    assert!(!is_kept(&item, &exclude));
}

#[test]
fn a_safe_parent_inherits_the_risk_of_an_item_inside_it() {
    let parent = safe_delete("/home/me/.cache", BIG);
    let child = sized_item(
        Category::AiModels,
        Risk::Review,
        BIG,
        Action::DeletePath(PathBuf::from("/home/me/.cache/huggingface")),
    );
    let mut items = vec![parent, child];
    inherit_child_risk(&mut items);
    assert_eq!((items[0].risk, items[0].preselected), (Risk::Review, false));
    assert_eq!(items[1].risk, Risk::Review);
}

#[test]
fn compare_items_sorts_by_category_then_risk_then_bytes_descending() {
    let build = |category, risk, bytes| sized_item(category, risk, bytes, Action::EmptyTrash);
    let mut items = [
        build(Category::Trash, Risk::Safe, 1),
        build(Category::Docker, Risk::Review, 50),
        build(Category::Docker, Risk::Safe, 5),
        build(Category::Docker, Risk::Safe, 7),
        build(Category::AgentWorktrees, Risk::Danger, 2),
    ];
    items.sort_by(compare_items);
    let expected = [
        (Category::AgentWorktrees, Risk::Danger, 2),
        (Category::Docker, Risk::Safe, 7),
        (Category::Docker, Risk::Safe, 5),
        (Category::Docker, Risk::Review, 50),
        (Category::Trash, Risk::Safe, 1),
    ];
    assert_eq!(sort_key(&items), expected);
}

#[test]
fn a_danger_child_makes_its_safe_parent_danger_with_typed_confirmation() {
    let parent = PathBuf::from("/home/me/project");
    let mut items = vec![
        delete_item(&parent, Risk::Safe, 1),
        delete_item(&parent.join("wt"), Risk::Danger, 1),
    ];
    inherit_child_risk(&mut items);
    assert_eq!(items[0].risk, Risk::Danger);
    assert!(!items[0].preselected);
    assert_eq!(
        items[0].typed_confirmation.as_deref(),
        Some(DELETE_CONFIRMATION)
    );
    assert_eq!(items[1].risk, Risk::Danger);
}

#[test]
fn equal_or_lower_child_risk_leaves_the_parent_untouched() {
    let parent = PathBuf::from("/home/me/project");
    let mut items = vec![
        delete_item(&parent, Risk::Review, 1),
        delete_item(&parent.join("a"), Risk::Review, 1),
        delete_item(&parent.join("b"), Risk::Safe, 1),
    ];
    inherit_child_risk(&mut items);
    assert_eq!(items[0].risk, Risk::Review);
    assert!(items[0].typed_confirmation.is_none());
}

#[test]
fn a_prune_record_inside_a_deleted_tree_does_not_raise_its_risk() {
    let parent = PathBuf::from("/home/me/project");
    let prune = Action::PruneWorktrees {
        repo: parent.join("repo"),
    };
    let record = sized_item(Category::AgentWorktrees, Risk::Danger, 0, prune);
    let mut items = vec![delete_item(&parent, Risk::Safe, 1), record];
    inherit_child_risk(&mut items);
    assert_eq!(items[0].risk, Risk::Safe);
}
