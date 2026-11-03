use crate::cli::Args;
use crate::config::{self, Config};
use crate::model::{Category, Item};
use crate::platform::{self, NO_HOME};
use crate::scan::{OnScannerFinished, ScanContext, scan_with_progress};
use crate::tools::Tools;
use anyhow::{Context, bail};
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};

const MAX_POOL_THREADS: usize = 8;

pub fn init_thread_pool() {
    let available = std::thread::available_parallelism().map_or(1, NonZeroUsize::get);
    let threads = available.min(MAX_POOL_THREADS);
    let _ = rayon::ThreadPoolBuilder::new()
        .num_threads(threads)
        .build_global();
}

pub(crate) fn home_dir() -> anyhow::Result<PathBuf> {
    platform::home_dir().context(NO_HOME)
}

pub(crate) fn load_settings(args: &Args) -> anyhow::Result<(PathBuf, Config)> {
    let home = home_dir()?;
    let mut config = config::load(args.config.as_deref(), &home)?;
    for root in &args.roots {
        config.dev_roots.push(checked_root(root, &home)?);
    }
    Ok((home, config))
}

fn checked_root(root: &Path, home: &Path) -> anyhow::Result<PathBuf> {
    let expanded = config::expand_tilde(root, home);
    let Some(folder) = std::fs::canonicalize(&expanded)
        .ok()
        .filter(|path| path.is_dir())
    else {
        bail!("--root {} is not a folder", root.display());
    };
    Ok(folder)
}

pub(crate) fn scan_with_tools(
    home: PathBuf,
    config: Config,
    only: &[Category],
) -> (ScanContext, Vec<Item>) {
    scan_with_tools_reporting(home, config, only, &|_| {})
}

pub(crate) fn scan_with_tools_reporting(
    home: PathBuf,
    config: Config,
    only: &[Category],
    on_finished: OnScannerFinished<'_>,
) -> (ScanContext, Vec<Item>) {
    let ctx = ScanContext {
        home,
        config,
        tools: Tools::discover(),
    };
    let items = scan_with_progress(&ctx, only, on_finished);
    (ctx, items)
}
