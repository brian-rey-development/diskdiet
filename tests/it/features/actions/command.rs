use diskdiet::domain::model::{Action, Outcome};
use diskdiet::features::actions::perform;
use diskdiet::infra::sys::tools::Tools;
use std::path::PathBuf;

fn run(program: &str, args: &[&str]) -> Outcome {
    let action = Action::RunCommand {
        program: PathBuf::from(program),
        args: args.iter().map(ToString::to_string).collect(),
    };
    perform(&action, &Tools::default())
}

#[test]
fn run_command_failure_is_reported_and_success_is_done() {
    assert!(matches!(run("/usr/bin/false", &[]), Outcome::Failed(_)));
    assert_eq!(run("/usr/bin/true", &[]), Outcome::Done);
}

#[test]
fn run_command_failure_carries_the_stderr_tail() {
    let outcome = run("/bin/sh", &["-c", "echo boom >&2; exit 3"]);
    assert_eq!(outcome, Outcome::Failed("boom".to_string()));
}

#[test]
fn run_command_with_missing_program_fails_without_panicking() {
    assert!(matches!(run("/nonexistent/tool", &[]), Outcome::Failed(_)));
}
