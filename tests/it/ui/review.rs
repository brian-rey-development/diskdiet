use console::strip_ansi_codes;
use diskdiet::model::{Action, Category, Item, Removal, Risk};
use diskdiet::ui::entries::Selection;
use diskdiet::ui::layout::note_row_budget;
use diskdiet::ui::review::{
    category_progress, progress_hint, render_review, review_groups, review_totals,
};
use std::path::PathBuf;

const MEGABYTE: u64 = 1_000_000;
const NARROW: usize = 80;
const WIDE: usize = 120;

fn worktree(repo: &str, branch: &str, risk: Risk) -> Item {
    let action = Action::RemoveWorktree {
        repo: PathBuf::from(format!("/home/dev/{repo}")),
        worktree: PathBuf::from(format!("/home/dev/{repo}/{branch}")),
        removal: Removal::Rechecked,
    };
    Item::new(
        Category::AgentWorktrees,
        branch,
        "",
        3 * MEGABYTE,
        risk,
        action,
    )
}

fn cache(name: &str, bytes: u64) -> Item {
    let action = Action::DeletePath(PathBuf::from(name));
    Item::new(Category::PackageCaches, name, "", bytes, Risk::Safe, action)
}

fn sample_items() -> Vec<Item> {
    vec![
        worktree("acme", "fix-login", Risk::Safe),
        worktree("acme", "spike", Risk::Danger),
        cache("pnpm", 40 * MEGABYTE),
        cache("npm", 10 * MEGABYTE),
    ]
}

fn picks(indices: &[usize]) -> Selection {
    indices.iter().copied().collect()
}

#[test]
fn groups_follow_category_order_and_hold_only_selected_items() {
    let items = sample_items();
    let groups = review_groups(&items, &picks(&[3, 1, 2]));
    let categories: Vec<Category> = groups.iter().map(|group| group.category).collect();
    assert_eq!(
        categories,
        [Category::AgentWorktrees, Category::PackageCaches]
    );
    assert_eq!(groups[0].entries.len(), 1);
    assert_eq!(groups[1].entries.len(), 2);
}

#[test]
fn totals_add_up_across_groups() {
    let items = sample_items();
    let groups = review_groups(&items, &picks(&[0, 1, 2, 3]));
    assert_eq!(groups[0].bytes(), 6 * MEGABYTE);
    assert_eq!(review_totals(&groups), (4, 56 * MEGABYTE));
    assert_eq!(review_totals(&review_groups(&items, &picks(&[]))), (0, 0));
}

#[test]
fn rendered_review_lists_every_item_with_subtotals_and_a_grand_total() {
    let items = sample_items();
    let groups = review_groups(&items, &picks(&[0, 1, 2]));
    let text = strip_ansi_codes(&render_review(&groups, NARROW)).into_owned();
    assert!(text.contains("Git worktrees · 2 items · 6.0 MB"));
    assert!(text.contains("acme › fix-login"));
    assert!(text.contains("DANGER acme › spike"));
    assert!(text.contains("Package manager caches · 1 item · 40.0 MB"));
    assert!(text.contains("Total · 3 items · 46.0 MB"));
    assert!(!text.contains("10.0 MB"));
}

#[test]
fn rendered_lines_never_exceed_the_note_budget() {
    let long_name = "x".repeat(300);
    let items = vec![
        cache(&long_name, MEGABYTE),
        worktree("acme", &long_name, Risk::Safe),
    ];
    let groups = review_groups(&items, &picks(&[0, 1]));
    for width in [NARROW, WIDE] {
        let text = strip_ansi_codes(&render_review(&groups, width)).into_owned();
        let widest = text.lines().map(|line| line.chars().count()).max();
        assert!(widest.is_some_and(|value| value <= note_row_budget(width)));
    }
}

#[test]
fn category_progress_counts_picked_items_per_available_category() {
    let items = sample_items();
    let progress = category_progress(&items, &picks(&[1, 2]));
    assert_eq!(progress.len(), 2);
    assert_eq!(progress_hint(&progress[0]), "1 of 2 selected · 3.0 MB");
    assert_eq!(progress_hint(&progress[1]), "1 of 2 selected · 40.0 MB");
}
