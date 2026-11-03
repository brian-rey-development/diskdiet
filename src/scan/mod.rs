pub mod apps;
pub mod builds;
pub mod caches;
pub mod docker;
pub mod downloads;
pub mod models;
mod pipeline;
pub(crate) mod support;
pub mod trash;
pub mod worktrees;

pub use pipeline::{compare_items, inherit_child_risk, is_kept};

use crate::{config::Config, log, model::Category, model::Item, tools::Tools};
use rayon::prelude::*;
use std::panic::{self, AssertUnwindSafe};
use std::path::{Path, PathBuf};

pub struct ScanContext {
    pub home: PathBuf,
    pub config: Config,
    pub tools: Tools,
}

pub type ScanFn = fn(&ScanContext) -> Vec<Item>;

pub struct Scanner {
    pub name: &'static str,
    pub scan: ScanFn,
}

pub const SCANNERS: &[Scanner] = &[
    Scanner {
        name: "worktrees",
        scan: worktrees::scan,
    },
    Scanner {
        name: "builds",
        scan: builds::scan,
    },
    Scanner {
        name: "caches",
        scan: caches::scan,
    },
    Scanner {
        name: "docker",
        scan: docker::scan,
    },
    Scanner {
        name: "models",
        scan: models::scan,
    },
    Scanner {
        name: "apps",
        scan: apps::scan,
    },
    Scanner {
        name: "downloads",
        scan: downloads::scan,
    },
    Scanner {
        name: "trash",
        scan: trash::scan,
    },
];

fn is_wanted(only: &[Category], category: Category) -> bool {
    only.is_empty() || only.contains(&category)
}

pub fn scan_all(ctx: &ScanContext) -> Vec<Item> {
    scan_only(ctx, &[])
}

fn with_panics_logged<T>(home: &Path, work: impl FnOnce() -> T) -> T {
    let home = home.to_path_buf();
    let previous = panic::take_hook();
    panic::set_hook(Box::new(move |info| {
        log::record(&home, &format!("scanner panicked and was skipped: {info}"));
    }));
    let result = work();
    panic::set_hook(previous);
    result
}

pub fn scan_only(ctx: &ScanContext, only: &[Category]) -> Vec<Item> {
    with_panics_logged(&ctx.home, || scan_selected(ctx, only))
}

fn scan_selected(ctx: &ScanContext, only: &[Category]) -> Vec<Item> {
    let mut items: Vec<Item> = SCANNERS
        .par_iter()
        .flat_map(|scanner| run_scanner(scanner.scan, ctx))
        .filter(|item| is_kept(item, &ctx.config.exclude))
        .collect();
    inherit_child_risk(&mut items);
    items.retain(|item| is_wanted(only, item.category));
    items.sort_by(compare_items);
    items
}

fn run_scanner(scan: ScanFn, ctx: &ScanContext) -> Vec<Item> {
    panic::catch_unwind(AssertUnwindSafe(|| scan(ctx))).unwrap_or_default()
}
