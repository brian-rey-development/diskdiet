mod problems;

use super::exec::Executed;
use super::{report, share};
use crate::app::cli::Args;
use crate::domain::model::{Action, Outcome, RunMode};
use crate::infra::fs::disk::{free_bytes, total_bytes};
use crate::infra::fs::size::format_bytes;
use crate::infra::util::fmt::count_noun;
use console::style;
use std::io;
use std::path::Path;

const PERCENT: u128 = 100;
const MIN_CELEBRATED_BYTES: u64 = 1_000_000;
const DONE: &str = "Done.";
const TRASH_HINT: &str = "Done. Items moved to the Trash free their space once you empty it.";

#[derive(Clone, Copy)]
struct FreeSpace {
    before: u64,
    after: u64,
}

impl FreeSpace {
    fn freed(self) -> u64 {
        self.after.saturating_sub(self.before)
    }
}

pub fn free_percent(free: u64, total: u64) -> u64 {
    if total == 0 {
        return 0;
    }
    let (free, total) = (u128::from(free), u128::from(total));
    let rounded = (free * PERCENT * 2 + total) / (total * 2);
    u64::try_from(rounded).unwrap_or(u64::MAX)
}

pub fn disk_status(free: u64, total: Option<u64>) -> String {
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

fn finish_dry_run(executed: &[Executed]) -> anyhow::Result<()> {
    let bytes: u64 = executed.iter().map(|entry| entry.item.bytes).sum();
    let line = format!(
        "Dry run complete. Up to {} could go. Nothing was deleted.",
        format_bytes(bytes)
    );
    cliclack::outro(line)?;
    Ok(())
}

fn celebrate(freed: u64, removed: usize) -> io::Result<()> {
    let big = style(format_bytes(freed)).green().bold();
    let removed_items = count_noun(removed, "item", "items");
    cliclack::note(
        "Diet complete",
        format!("{big} freed\n{removed_items} removed"),
    )
}

fn print_share(space: FreeSpace) -> anyhow::Result<()> {
    let freed = space.freed();
    let card = share::card(freed, space.before, space.after);
    let text = format!("\n{card}\n\nBrag about it: {}\n", share::tweet_url(freed));
    report::write_ignoring_closed_pipe(&text)
}

pub fn finish(
    args: &Args,
    home: &Path,
    executed: &[Executed],
    before_free: u64,
) -> anyhow::Result<()> {
    problems::report_problems(executed, home)?;
    if args.run_mode() == RunMode::DryRun {
        return finish_dry_run(executed);
    }
    let space = FreeSpace {
        before: before_free,
        after: free_bytes(home)?,
    };
    let status = disk_status(space.after, total_bytes(home));
    let removed = problems::count_done(executed);
    if removed == 0 {
        cliclack::outro(format!("Nothing was removed. {status}"))?;
        return Ok(());
    }
    finish_removed(executed, args, removed, space, &status)
}

fn moved_to_trash(executed: &[Executed]) -> bool {
    executed.iter().any(|entry| {
        entry.outcome == Outcome::Done && matches!(entry.item.action, Action::MoveToTrash(_))
    })
}

fn finish_removed(
    executed: &[Executed],
    args: &Args,
    removed: usize,
    space: FreeSpace,
    status: &str,
) -> anyhow::Result<()> {
    let freed = space.freed();
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
        print_share(space)?;
    }
    Ok(())
}
