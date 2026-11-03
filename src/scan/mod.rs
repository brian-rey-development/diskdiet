pub mod apps;
pub mod builds;
pub mod caches;
pub mod docker;
pub mod downloads;
pub mod models;
pub mod trash;
pub mod worktrees;

use crate::{
    config::Config,
    fsutil::lowercased,
    model::{Action, Category, DELETE_CONFIRMATION, Item, Risk},
    tools::Tools,
};
use rayon::prelude::*;
use std::path::{Path, PathBuf};

const MIN_ITEM_BYTES: u64 = 1_000_000;

pub struct ScanContext {
    pub home: PathBuf,
    pub config: Config,
    pub tools: Tools,
}

pub type ScanFn = fn(&ScanContext) -> Vec<Item>;

pub struct Scanner {
    pub name: &'static str,
    pub categories: &'static [Category],
    pub scan: ScanFn,
}

pub const SCANNERS: &[Scanner] = &[
    Scanner {
        name: "worktrees",
        categories: &[Category::AgentWorktrees],
        scan: worktrees::scan,
    },
    Scanner {
        name: "builds",
        categories: &[Category::BuildArtifacts, Category::PythonEnvs],
        scan: builds::scan,
    },
    Scanner {
        name: "caches",
        categories: &[Category::PackageCaches, Category::AppCaches],
        scan: caches::scan,
    },
    Scanner {
        name: "docker",
        categories: &[Category::Docker],
        scan: docker::scan,
    },
    Scanner {
        name: "models",
        categories: &[Category::AiModels],
        scan: models::scan,
    },
    Scanner {
        name: "apps",
        categories: &[Category::UnusedApps],
        scan: apps::scan,
    },
    Scanner {
        name: "downloads",
        categories: &[Category::Downloads],
        scan: downloads::scan,
    },
    Scanner {
        name: "trash",
        categories: &[Category::Trash],
        scan: trash::scan,
    },
];

fn wanted(only: &[Category], category: Category) -> bool {
    only.is_empty() || only.contains(&category)
}

fn is_selected(scanner: &Scanner, only: &[Category]) -> bool {
    only.is_empty()
        || scanner
            .categories
            .iter()
            .any(|category| only.contains(category))
}

pub fn scan_all(ctx: &ScanContext) -> Vec<Item> {
    scan_only(ctx, &[])
}

pub(crate) fn scan_only(ctx: &ScanContext, only: &[Category]) -> Vec<Item> {
    let home = ctx.home.clone();
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        crate::log::record(&home, &format!("scanner panicked and was skipped: {info}"));
    }));
    let items = scan_quietly(ctx, only);
    std::panic::set_hook(previous);
    items
}

fn scan_quietly(ctx: &ScanContext, only: &[Category]) -> Vec<Item> {
    let mut items: Vec<Item> = SCANNERS
        .par_iter()
        .filter(|scanner| is_selected(scanner, only))
        .flat_map(|scanner| run_scanner(scanner.scan, ctx))
        .filter(|item| wanted(only, item.category) && is_kept(item, &ctx.config.exclude))
        .collect();
    inherit_child_risk(&mut items);
    items.sort_by(compare_items);
    items
}

fn covering_path(action: &Action) -> Option<&Path> {
    action.path().filter(|_| action.deletes_path())
}

fn nested_path(action: &Action) -> Option<&Path> {
    match action {
        Action::PruneWorktrees { .. } => None,
        other => other.path(),
    }
}

fn riskiest_child(items: &[Item], parent: &Path) -> Option<Risk> {
    items
        .iter()
        .filter_map(|item| nested_path(&item.action).map(|path| (path, item.risk)))
        .filter(|(path, _)| *path != parent && path.starts_with(parent))
        .map(|(_, risk)| risk)
        .max()
}

pub fn inherit_child_risk(items: &mut [Item]) {
    let raised: Vec<Option<Risk>> = items
        .iter()
        .map(|item| covering_path(&item.action).and_then(|path| riskiest_child(items, path)))
        .map(|child| child.filter(|risk| *risk > Risk::Safe))
        .collect();
    for (item, child) in items.iter_mut().zip(raised) {
        let Some(child) = child.filter(|risk| *risk > item.risk) else {
            continue;
        };
        item.risk = child;
        item.preselected = false;
        if child == Risk::Danger {
            item.typed_confirmation = Some(DELETE_CONFIRMATION.to_owned());
        }
    }
}

fn run_scanner(scan: ScanFn, ctx: &ScanContext) -> Vec<Item> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| scan(ctx))).unwrap_or_default()
}

pub fn compare_items(a: &Item, b: &Item) -> std::cmp::Ordering {
    a.category
        .cmp(&b.category)
        .then(a.risk.cmp(&b.risk))
        .then(b.bytes.cmp(&a.bytes))
        .then_with(|| a.label.cmp(&b.label))
}

pub fn is_kept(item: &Item, exclude: &[PathBuf]) -> bool {
    if item.bytes < MIN_ITEM_BYTES && !item.action.ignores_size_floor() {
        return false;
    }
    let Some(path) = item.action.path().map(lowercased) else {
        return true;
    };
    !exclude
        .iter()
        .map(|excluded| lowercased(excluded))
        .any(|excluded| path.starts_with(&excluded) || excluded.starts_with(&path))
}
