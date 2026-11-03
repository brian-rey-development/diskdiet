use super::layout::{spinner_row_budget, terminal_width};
use super::scan_progress::ScanProgress;
use crate::cli::Args;
use crate::config::Config;
use crate::fmt::count_noun;
use crate::model::{Category, Item, RunMode};
use crate::process::run_stdout;
use crate::scan::ScanContext;
use crate::scan::{SCANNERS, ScannerReport};
use crate::session::scan_with_tools_reporting;
use crate::size::format_bytes;
use crate::tools;
use console::style;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, PoisonError};
use std::time::Duration;

const WEIGHING: &str = "Weighing your disk...";
const DOCKER_SKIPPED: &str = "Docker is not running, skipped";
const LOW_SPACE_WARNING: &str = "You are running low on space. Time for a diet.";
const BYTES_PER_GB: u64 = 1_000_000_000;
const BRAND_LIME: u8 = 191;
const BRAND_INK: u8 = 16;
const DOCKER_INFO_TIMEOUT: Duration = Duration::from_secs(10);

pub(super) fn show_intro(args: &Args, before_free: u64, low_space_gb: u64) -> io::Result<()> {
    let tag = match args.run_mode() {
        RunMode::DryRun => " diskdiet (dry run) ",
        RunMode::Live => " diskdiet ",
    };
    cliclack::intro(
        style(tag)
            .color256(BRAND_INK)
            .on_color256(BRAND_LIME)
            .bold(),
    )?;
    cliclack::log::info(format!(
        "{} free right now",
        style(format_bytes(before_free)).bold()
    ))?;
    if before_free < low_space_gb.saturating_mul(BYTES_PER_GB) {
        cliclack::log::warning(LOW_SPACE_WARNING)?;
    }
    Ok(())
}

pub fn found_summary(total_bytes: u64, item_count: usize) -> String {
    let items = count_noun(item_count, "item", "items");
    format!("Found {} across {items}", format_bytes(total_bytes))
}

pub(super) fn scan_with_spinner(
    home: PathBuf,
    config: Config,
    only: &[Category],
) -> (ScanContext, Vec<Item>) {
    let spinner = cliclack::spinner();
    spinner.start(WEIGHING);
    let progress = Mutex::new(ScanProgress::new(SCANNERS.len(), only));
    let width = spinner_row_budget(terminal_width());
    let on_finished = |report: ScannerReport<'_>| {
        let mut progress = progress.lock().unwrap_or_else(PoisonError::into_inner);
        progress.record(report.items);
        spinner.set_message(progress.line(width));
    };
    let (ctx, items) = scan_with_tools_reporting(home, config, only, &on_finished);
    let total: u64 = items.iter().map(|item| item.bytes).sum();
    spinner.stop(found_summary(total, items.len()));
    (ctx, items)
}

pub(super) fn docker_note(ctx: &ScanContext, items: &[Item], only: &[Category]) -> io::Result<()> {
    if !only.is_empty() && !only.contains(&Category::Docker) {
        return Ok(());
    }
    let has_docker_items = items.iter().any(|item| item.category == Category::Docker);
    let Some(docker) = ctx.tools.get(tools::DOCKER) else {
        return Ok(());
    };
    if has_docker_items || docker_is_running(docker) {
        return Ok(());
    }
    cliclack::log::remark(DOCKER_SKIPPED)
}

fn docker_is_running(docker: &Path) -> bool {
    run_stdout(docker, &["info"], DOCKER_INFO_TIMEOUT).is_some()
}
