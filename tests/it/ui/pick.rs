use crate::support::fixtures::fixture_item;
use diskdiet::model::{Action, Risk, RunMode};
use diskdiet::ui::pick::{confirm_prompt, item_label, starts_selected};

#[test]
fn danger_items_are_marked_and_never_start_selected() {
    let mut danger = fixture_item(Action::EmptyTrash);
    danger.risk = Risk::Danger;
    danger.typed_confirmation = Some("delete".to_string());
    assert!(!starts_selected(&danger));
    assert!(item_label(&danger).contains("DANGER"));
    assert!(starts_selected(&fixture_item(Action::EmptyTrash)));
}

#[test]
fn confirm_prompt_wording_changes_in_dry_run() {
    assert_eq!(
        confirm_prompt(3, 0, RunMode::DryRun),
        "Dry run: nothing will be deleted. Continue?"
    );
    assert!(confirm_prompt(3, 0, RunMode::Live).starts_with("Delete 3 items ("));
}
