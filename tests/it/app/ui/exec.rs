use crate::support::fixtures::fixture_item;
use diskdiet::app::ui::exec::{execution_order, processed_summary};
use diskdiet::domain::model::Action;
use std::path::PathBuf;

fn run_command(args: &[&str]) -> Action {
    Action::RunCommand {
        program: PathBuf::from("/usr/bin/true"),
        args: args.iter().map(ToString::to_string).collect(),
    }
}

#[test]
fn execution_order_empties_the_trash_first_and_trims_the_vm_last() {
    let trash = fixture_item(Action::EmptyTrash);
    let trim = fixture_item(run_command(&["ssh", "--", "sudo", "fstrim", "-a"]));
    let prune = fixture_item(run_command(&["image", "prune", "-af"]));
    let app = fixture_item(Action::MoveToTrash(PathBuf::from("/Applications/X.app")));
    let ordered = execution_order(&[&trash, &trim, &prune, &app]);
    let actions: Vec<&Action> = ordered.iter().map(|item| &item.action).collect();
    assert_eq!(
        actions,
        vec![&trash.action, &prune.action, &app.action, &trim.action]
    );
}

#[test]
fn processed_summary_says_item_for_one_and_items_otherwise() {
    assert_eq!(processed_summary(0), "Processed 0 items");
    assert_eq!(processed_summary(1), "Processed 1 item");
    assert_eq!(processed_summary(2), "Processed 2 items");
}
