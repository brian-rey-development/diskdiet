use diskdiet::model::{Action, Category, Item, Removal, Risk};
use diskdiet::ui::entries::{category_entries, display_label, item_label};
use std::path::PathBuf;

const MEGABYTE: u64 = 1_000_000;

fn worktree(repo: &str, branch: &str) -> Item {
    let action = Action::RemoveWorktree {
        repo: PathBuf::from(format!("/home/dev/{repo}")),
        worktree: PathBuf::from(format!("/home/dev/{repo}/.claude/worktrees/{branch}")),
        removal: Removal::Rechecked,
    };
    let category = Category::AgentWorktrees;
    Item::new(category, branch, "", MEGABYTE, Risk::Safe, action)
}

fn build_dir(name: &str, bytes: u64) -> Item {
    let action = Action::DeletePath(PathBuf::from(name));
    Item::new(
        Category::BuildArtifacts,
        name,
        "",
        bytes,
        Risk::Safe,
        action,
    )
}

fn labels_in_order(items: &[Item], category: Category) -> Vec<String> {
    let entries = category_entries(items, category);
    entries.iter().map(|(_, item)| item.label.clone()).collect()
}

#[test]
fn worktrees_are_grouped_by_repo_and_then_sorted_by_label() {
    let items = vec![
        worktree("zeta", "b-branch"),
        worktree("alpha", "z-branch"),
        worktree("zeta", "A-branch"),
        worktree("alpha", "a-branch"),
    ];
    assert_eq!(
        labels_in_order(&items, Category::AgentWorktrees),
        ["a-branch", "z-branch", "A-branch", "b-branch"]
    );
}

#[test]
fn sorted_entries_keep_the_original_item_indices() {
    let items = vec![worktree("zeta", "one"), worktree("alpha", "two")];
    let entries = category_entries(&items, Category::AgentWorktrees);
    let indices: Vec<usize> = entries.iter().map(|(index, _)| *index).collect();
    assert_eq!(indices, [1, 0]);
}

#[test]
fn other_categories_keep_the_scan_order_and_skip_foreign_items() {
    let items = vec![
        build_dir("small", 1),
        worktree("alpha", "branch"),
        build_dir("large", 2),
    ];
    assert_eq!(
        labels_in_order(&items, Category::BuildArtifacts),
        ["small", "large"]
    );
}

#[test]
fn worktree_labels_name_the_repo_and_other_labels_are_untouched() {
    assert_eq!(
        display_label(&worktree("acme-app", "fix-login")),
        "acme-app › fix-login"
    );
    assert_eq!(display_label(&build_dir("target", 1)), "target");
}

#[test]
fn danger_rows_carry_the_tag_and_the_repo() {
    let mut risky = worktree("acme-app", "fix-login");
    risky.risk = Risk::Danger;
    let row = console::strip_ansi_codes(&item_label(&risky)).into_owned();
    assert!(row.contains("DANGER acme-app › fix-login"));
}
