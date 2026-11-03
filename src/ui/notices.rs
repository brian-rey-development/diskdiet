use crate::cli::Args;
use crate::config::Config;
use crate::model::{Category, Item, RunMode};
use crate::process::run_stdout;
use crate::scan::ScanContext;
use crate::session::scan_with_tools;
use crate::size::format_bytes;
use crate::tools;
use console::style;
use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

const WEIGHING: &str = "Weighing your disk...";
const DOCKER_SKIPPED: &str = "Docker is not running, skipped";
const LOW_SPACE_WARNING: &str = "You are running low on space. Time for a diet.";
const BYTES_PER_GB: u64 = 1_000_000_000;
const DOCKER_INFO_TIMEOUT: Duration = Duration::from_secs(10);

pub(super) fn show_intro(args: &Args, before_free: u64, low_space_gb: u64) -> io::Result<()> {
    let tag = match args.run_mode() {
        RunMode::DryRun => " diskdiet (dry run) ",
        RunMode::Live => " diskdiet ",
    };
    cliclack::intro(style(tag).reverse().bold())?;
    cliclack::log::info(format!(
        "{} free right now",
        style(format_bytes(before_free)).bold()
    ))?;
    if before_free < low_space_gb.saturating_mul(BYTES_PER_GB) {
        cliclack::log::warning(LOW_SPACE_WARNING)?;
    }
    Ok(())
}

pub(super) fn scan_with_spinner(
    home: PathBuf,
    config: Config,
    only: &[Category],
) -> (ScanContext, Vec<Item>) {
    let spinner = cliclack::spinner();
    spinner.start(WEIGHING);
    let (ctx, items) = scan_with_tools(home, config, only);
    let total: u64 = items.iter().map(|item| item.bytes).sum();
    spinner.stop(format!(
        "Found {} across {} items",
        format_bytes(total),
        items.len()
    ));
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
