pub mod exec;
pub mod finish;
pub mod pick;
pub mod report;
pub mod share;

use crate::cli::Args;
use crate::config::{self, Config};
use crate::disk::free_bytes;
use crate::model::{Category, Item};
use crate::scan::{ScanContext, scan_only};
use crate::size::format_bytes;
use crate::tools::Tools;
use crate::tools::run_with_deadline;
use console::style;
use std::io;
use std::path::PathBuf;
use std::process::Command;

const CANCEL_MESSAGE: &str = "Diet postponed. Nothing was deleted.";
const NOTHING_TO_TRIM: &str = "Nothing to trim. Your disk is already lean.";
const NOTHING_SELECTED: &str = "Nothing selected. Your disk stays exactly as it is.";
const WEIGHING: &str = "Weighing your disk...";
const DOCKER_SKIPPED: &str = "Docker is not running, skipped";
const BYTES_PER_GB: u64 = 1_000_000_000;
const DOCKER_INFO_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

pub fn run(args: Args) -> anyhow::Result<()> {
    if args.report {
        return report::run(&args);
    }
    match interactive(&args) {
        Err(error) if is_interrupt(&error) => {
            cliclack::outro_cancel(CANCEL_MESSAGE)?;
            Ok(())
        }
        other => other,
    }
}

pub fn is_interrupt(error: &anyhow::Error) -> bool {
    error
        .downcast_ref::<io::Error>()
        .is_some_and(|e| e.kind() == io::ErrorKind::Interrupted)
}

pub fn home_dir() -> anyhow::Result<PathBuf> {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or_else(|| anyhow::anyhow!("HOME is not set"))
}

pub fn load_settings(args: &Args) -> anyhow::Result<(PathBuf, Config)> {
    let home = home_dir()?;
    let mut config = config::load(args.config.as_deref(), &home)?;
    for root in &args.roots {
        config.dev_roots.push(checked_root(root, &home)?);
    }
    Ok((home, config))
}

fn checked_root(root: &std::path::Path, home: &std::path::Path) -> anyhow::Result<PathBuf> {
    let expanded = config::expand_tilde(root, home);
    std::fs::canonicalize(&expanded)
        .ok()
        .filter(|path| path.is_dir())
        .ok_or_else(|| anyhow::anyhow!("--root {} is not a folder", root.display()))
}

pub fn scan_with_tools(
    home: PathBuf,
    config: Config,
    only: &[Category],
) -> (ScanContext, Vec<Item>) {
    let ctx = ScanContext {
        home,
        config,
        tools: Tools::discover(),
    };
    let items = scan_only(&ctx, only);
    (ctx, items)
}

fn interactive(args: &Args) -> anyhow::Result<()> {
    let (home, config) = load_settings(args)?;
    let low_space_gb = config.low_space_warning_gb;
    open(args, free_bytes(&home)?, low_space_gb)?;
    let (ctx, items) = weigh(home, config, &args.only);
    if items.is_empty() {
        cliclack::outro(NOTHING_TO_TRIM)?;
        return Ok(());
    }
    summary(&items)?;
    docker_note(&ctx, &items, &args.only)?;
    let chosen = pick::pick_all(&items)?;
    let selected: Vec<&Item> = chosen.iter().map(|&index| &items[index]).collect();
    execute_selection(args, &ctx, &selected)
}

fn execute_selection(args: &Args, ctx: &ScanContext, selected: &[&Item]) -> anyhow::Result<()> {
    if selected.is_empty() {
        cliclack::outro(NOTHING_SELECTED)?;
        return Ok(());
    }
    if !pick::confirm_all(selected, args.dry_run)? {
        cliclack::outro_cancel(CANCEL_MESSAGE)?;
        return Ok(());
    }
    let before_free = free_bytes(&ctx.home)?;
    let executed = exec::execute_all(selected, &ctx.tools, args.dry_run);
    finish::finish(args, &ctx.home, &executed, before_free)
}

fn open(args: &Args, before_free: u64, low_space_gb: u64) -> io::Result<()> {
    let tag = if args.dry_run {
        " diskdiet (dry run) "
    } else {
        " diskdiet "
    };
    cliclack::intro(style(tag).reverse().bold())?;
    cliclack::log::info(format!(
        "{} free right now",
        style(format_bytes(before_free)).bold()
    ))?;
    if before_free < low_space_gb.saturating_mul(BYTES_PER_GB) {
        cliclack::log::warning("You are running low on space. Time for a diet.")?;
    }
    Ok(())
}

fn weigh(home: PathBuf, config: Config, only: &[Category]) -> (ScanContext, Vec<Item>) {
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

struct CategoryRow {
    category: Category,
    bytes: u64,
    count: usize,
}

fn category_rows(items: &[Item]) -> Vec<CategoryRow> {
    let mut rows: Vec<CategoryRow> = Category::ALL
        .iter()
        .map(|&category| {
            let own = items.iter().filter(|item| item.category == category);
            CategoryRow {
                category,
                bytes: own.clone().map(|i| i.bytes).sum(),
                count: own.count(),
            }
        })
        .filter(|row| row.count > 0)
        .collect();
    rows.sort_by_key(|row| std::cmp::Reverse(row.bytes));
    rows
}

fn render_rows(rows: &[CategoryRow]) -> String {
    let width = rows
        .iter()
        .map(|r| r.category.title().len())
        .max()
        .unwrap_or(0);
    let lines: Vec<String> = rows
        .iter()
        .map(|row| {
            let noun = if row.count == 1 { "item" } else { "items" };
            let size = format!("{:>9}", format_bytes(row.bytes));
            let title = format!("{:<width$}", row.category.title());
            format!(
                "{title}  {}  {}",
                style(size).bold(),
                style(format!("{} {noun}", row.count)).dim()
            )
        })
        .collect();
    lines.join("\n")
}

fn summary(items: &[Item]) -> io::Result<()> {
    let total: u64 = items.iter().map(|item| item.bytes).sum();
    let title = format!(
        "You can lose up to {}",
        style(format_bytes(total)).bold().green()
    );
    cliclack::note(title, render_rows(&category_rows(items)))
}

fn docker_note(ctx: &ScanContext, items: &[Item], only: &[Category]) -> io::Result<()> {
    if !only.is_empty() && !only.contains(&Category::Docker) {
        return Ok(());
    }
    let has_docker_items = items.iter().any(|item| item.category == Category::Docker);
    let Some(docker) = ctx.tools.get("docker") else {
        return Ok(());
    };
    if has_docker_items || docker_is_running(docker) {
        return Ok(());
    }
    cliclack::log::remark(DOCKER_SKIPPED)
}

fn docker_is_running(docker: &std::path::Path) -> bool {
    let mut command = Command::new(docker);
    command.arg("info");
    run_with_deadline(&mut command, DOCKER_INFO_TIMEOUT).is_some_and(|(status, _)| status.success())
}
