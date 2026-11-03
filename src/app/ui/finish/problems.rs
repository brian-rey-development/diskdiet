use crate::app::ui::exec::Executed;
use crate::domain::model::Outcome;
use crate::infra::sys::platform::{self, IS_MACOS};
use crate::infra::util::fmt::{count_noun, tilde_path};
use std::io;
use std::path::Path;

const MAX_LISTED_FAILURES: usize = 5;
const PERMISSION_STEPS: &str = "Some items belong to another user or sit in a read-only folder.\n\
Check their owner with ls -l, then remove them by hand if you are sure.";
const FDA_STEPS: &str = "macOS blocked some items.\n\
1. Open System Settings > Privacy & Security > Full Disk Access\n\
2. Switch on your terminal app\n\
3. Quit and reopen the terminal, then run diskdiet again\n\
\n\
If a prompt asks to let your terminal control Finder, allow it.";

fn count_outcome(executed: &[Executed], outcome: &Outcome) -> usize {
    executed
        .iter()
        .filter(|entry| entry.outcome == *outcome)
        .count()
}

fn report_stopped(executed: &[Executed]) -> io::Result<()> {
    let stopped = count_outcome(executed, &Outcome::Stopped);
    if stopped == 0 {
        return Ok(());
    }
    let subject = count_noun(stopped, "item was", "items were");
    cliclack::log::warning(format!("Stopped with Ctrl-C. {subject} left untouched."))
}

fn problem_lines(executed: &[Executed]) -> Vec<String> {
    executed
        .iter()
        .filter_map(|entry| match &entry.outcome {
            Outcome::Failed(why) => Some(format!("{}: {why}", entry.item.label)),
            Outcome::Skipped(why) => Some(format!("{}: skipped, {why}", entry.item.label)),
            _ => None,
        })
        .collect()
}

fn report_failures(executed: &[Executed], home: &Path) -> io::Result<()> {
    let lines = problem_lines(executed);
    for line in lines.iter().take(MAX_LISTED_FAILURES) {
        cliclack::log::warning(line)?;
    }
    let hidden = lines.len().saturating_sub(MAX_LISTED_FAILURES);
    if hidden > 0 {
        let log_file = tilde_path(&platform::log_path(home), home);
        cliclack::log::warning(format!("{hidden} more in {log_file}"))?;
    }
    Ok(())
}

fn report_permission_block() -> io::Result<()> {
    let (title, steps) = if IS_MACOS {
        ("Full Disk Access needed", FDA_STEPS)
    } else {
        ("Permission denied", PERMISSION_STEPS)
    };
    cliclack::note(title, steps)
}

pub(super) fn report_problems(executed: &[Executed], home: &Path) -> io::Result<()> {
    report_failures(executed, home)?;
    report_stopped(executed)?;
    if count_outcome(executed, &Outcome::NeedsFullDiskAccess) > 0 {
        report_permission_block()?;
    }
    Ok(())
}

pub(super) fn count_done(executed: &[Executed]) -> usize {
    count_outcome(executed, &Outcome::Done)
}
