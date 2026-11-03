pub mod entries;
pub mod exec;
mod finish;
pub mod layout;
pub mod notices;
pub mod pick;
pub mod report;
pub mod review;
pub mod scan_progress;
pub mod share;
mod summary;
mod theme;

use crate::app::cli::Args;
use crate::app::session;
use crate::domain::model::{Category, Item};
use crate::features::scan::ScanContext;
use crate::infra::fs::disk::free_bytes;
use std::io;

const CANCEL_MESSAGE: &str = "Diet postponed. Nothing was deleted.";
const NOTHING_TO_TRIM: &str = "Nothing to trim. Your disk is already lean.";
const NOTHING_IN_CHOSEN: &str = "Nothing to trim in the categories you chose.";
const NOTHING_SELECTED: &str = "Nothing selected. Your disk stays exactly as it is.";

pub fn run(args: &Args) -> anyhow::Result<()> {
    if args.report {
        return report::run(args);
    }
    theme::install();
    match interactive(args) {
        Err(error) if is_interrupt(&error) => {
            cliclack::outro_cancel(CANCEL_MESSAGE)?;
            Ok(())
        }
        other => other,
    }
}

fn nothing_found_message(only: &[Category]) -> &'static str {
    if only.is_empty() {
        NOTHING_TO_TRIM
    } else {
        NOTHING_IN_CHOSEN
    }
}

fn is_interrupt(error: &anyhow::Error) -> bool {
    error
        .downcast_ref::<io::Error>()
        .is_some_and(|cause| cause.kind() == io::ErrorKind::Interrupted)
}

fn interactive(args: &Args) -> anyhow::Result<()> {
    let (home, config) = session::load_settings(args)?;
    let low_space_gb = config.low_space_warning_gb;
    notices::show_intro(args, free_bytes(&home)?, low_space_gb)?;
    let (ctx, items) = notices::scan_with_spinner(home, config, &args.only);
    if items.is_empty() {
        cliclack::outro(nothing_found_message(&args.only))?;
        return Ok(());
    }
    summary::show_summary(&items)?;
    notices::docker_note(&ctx, &items, &args.only)?;
    let Some(chosen) = pick::choose(&items)? else {
        cliclack::outro_cancel(CANCEL_MESSAGE)?;
        return Ok(());
    };
    let selected: Vec<&Item> = chosen.iter().map(|&index| &items[index]).collect();
    execute_selection(args, &ctx, &selected)
}

fn execute_selection(args: &Args, ctx: &ScanContext, selected: &[&Item]) -> anyhow::Result<()> {
    if selected.is_empty() {
        cliclack::outro(NOTHING_SELECTED)?;
        return Ok(());
    }
    if !pick::confirm_all(selected, args.run_mode())? {
        cliclack::outro_cancel(CANCEL_MESSAGE)?;
        return Ok(());
    }
    let before_free = free_bytes(&ctx.home)?;
    let executed = exec::execute_all(selected, &ctx.tools, args.run_mode());
    finish::finish(args, &ctx.home, &executed, before_free)
}
