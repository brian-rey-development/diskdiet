use super::exec::Executed;
use super::share;
use crate::cli::Args;
use crate::disk::{free_bytes, total_bytes};
use crate::fmt::{count_noun, tilde_path};
use crate::model::{Action, Outcome, RunMode};
use crate::platform::{self, IS_MACOS};
use crate::size::format_bytes;
use console::style;
use std::path::Path;

const MAX_LISTED_FAILURES: usize = 5;
const PERCENT: f64 = 100.0;
const MIN_CELEBRATED_BYTES: u64 = 1_000_000;
const DONE: &str = "Done.";
const TRASH_HINT: &str = "Done. Items moved to the Trash free their space once you empty it.";
const PERMISSION_STEPS: &str = "Some items belong to another user or sit in a read-only folder.\n\
Check their owner with ls -l, then remove them by hand if you are sure.";
const FDA_STEPS: &str = "macOS blocked some items.\n\
1. Open System Settings > Privacy & Security > Full Disk Access\n\
2. Switch on your terminal app\n\
3. Quit and reopen the terminal, then run diskdiet again\n\
\n\
If a prompt asks to let your terminal control Finder, allow it.";

pub(crate) fn free_percent(free: u64, total: u64) -> u64 {
    if total == 0 {
        return 0;
    }
    (free as f64 / total as f64 * PERCENT).round() as u64
}

pub(crate) fn disk_status(free: u64, total: Option<u64>) -> String {
    let Some(total) = total else {
        return format!("Your disk now has {} free", format_bytes(free));
    };
    let used = total.saturating_sub(free);
    let pct = free_percent(free, total);
    format!(
        "Your disk is now {}/{} ({pct}% free)",
        format_bytes(used),
        format_bytes(total)
    )
}

fn report_stopped(executed: &[Executed]) -> std::io::Result<()> {
    let stopped = executed
        .iter()
        .filter(|e| e.outcome == Outcome::Stopped)
        .count();
    if stopped == 0 {
        return Ok(());
    }
    let subject = count_noun(stopped, "item was", "items were");
    cliclack::log::warning(format!("Stopped with Ctrl-C. {subject} left untouched."))
}

fn problem_lines(executed: &[Executed]) -> Vec<String> {
    executed
        .iter()
        .filter_map(|e| match &e.outcome {
            Outcome::Failed(why) => Some(format!("{}: {why}", e.item.label)),
            Outcome::Skipped(why) => Some(format!("{}: skipped, {why}", e.item.label)),
            _ => None,
        })
        .collect()
}

fn report_failures(executed: &[Executed], home: &Path) -> std::io::Result<()> {
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

fn report_problems(executed: &[Executed], home: &Path) -> std::io::Result<()> {
    report_failures(executed, home)?;
    report_stopped(executed)?;
    let blocked = executed
        .iter()
        .any(|e| e.outcome == Outcome::NeedsFullDiskAccess);
    if blocked {
        let (title, steps) = if IS_MACOS {
            ("Full Disk Access needed", FDA_STEPS)
        } else {
            ("Permission denied", PERMISSION_STEPS)
        };
        cliclack::note(title, steps)?;
    }
    Ok(())
}

fn finish_dry_run(executed: &[Executed]) -> anyhow::Result<()> {
    let bytes: u64 = executed.iter().map(|e| e.item.bytes).sum();
    let line = format!(
        "Dry run complete. Up to {} could go. Nothing was deleted.",
        format_bytes(bytes)
    );
    cliclack::outro(line)?;
    Ok(())
}

fn count_done(executed: &[Executed]) -> usize {
    executed
        .iter()
        .filter(|e| e.outcome == Outcome::Done)
        .count()
}

fn celebrate(freed: u64, removed: usize) -> std::io::Result<()> {
    let big = style(format_bytes(freed)).green().bold();
    let removed_items = count_noun(removed, "item", "items");
    cliclack::note(
        "Diet complete",
        format!("{big} freed\n{removed_items} removed"),
    )
}

fn print_share(freed: u64, before_free: u64, after_free: u64) -> anyhow::Result<()> {
    let card = share::card(freed, before_free, after_free);
    let text = format!("\n{card}\n\nBrag about it: {}\n", share::tweet_url(freed));
    super::report::write_ignoring_closed_pipe(&text)
}

pub(crate) fn finish(
    args: &Args,
    home: &Path,
    executed: &[Executed],
    before_free: u64,
) -> anyhow::Result<()> {
    report_problems(executed, home)?;
    if args.run_mode() == RunMode::DryRun {
        return finish_dry_run(executed);
    }
    let after_free = free_bytes(home)?;
    let status = disk_status(after_free, total_bytes(home));
    let removed = count_done(executed);
    if removed == 0 {
        cliclack::outro(format!("Nothing was removed. {status}"))?;
        return Ok(());
    }
    finish_removed(executed, args, removed, (before_free, after_free), &status)
}

fn moved_to_trash(executed: &[Executed]) -> bool {
    executed
        .iter()
        .any(|e| e.outcome == Outcome::Done && matches!(e.item.action, Action::MoveToTrash(_)))
}

fn finish_removed(
    executed: &[Executed],
    args: &Args,
    removed: usize,
    (before_free, after_free): (u64, u64),
    status: &str,
) -> anyhow::Result<()> {
    let freed = after_free.saturating_sub(before_free);
    if freed < MIN_CELEBRATED_BYTES {
        let hint = if moved_to_trash(executed) {
            TRASH_HINT
        } else {
            DONE
        };
        cliclack::outro(format!("{hint} {status}"))?;
        return Ok(());
    }
    celebrate(freed, removed)?;
    cliclack::outro(format!(
        "Lost {}. {status}",
        style(format_bytes(freed)).bold().green()
    ))?;
    if !args.no_share {
        print_share(freed, before_free, after_free)?;
    }
    Ok(())
}
