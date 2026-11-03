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
    model::{Category, Item},
    tools::Tools,
};
use std::path::PathBuf;

const MIN_ITEM_BYTES: u64 = 1_000_000;

pub struct ScanContext {
    pub home: PathBuf,
    pub config: Config,
    pub tools: Tools,
}

pub type ScanFn = fn(&ScanContext) -> Vec<Item>;

pub const SCANNERS: [(&str, ScanFn); 8] = [
    ("worktrees", worktrees::scan),
    ("builds", builds::scan),
    ("caches", caches::scan),
    ("docker", docker::scan),
    ("models", models::scan),
    ("apps", apps::scan),
    ("downloads", downloads::scan),
    ("trash", trash::scan),
];

pub fn scanner_categories(name: &str) -> &'static [Category] {
    match name {
        "worktrees" => &[Category::AgentWorktrees],
        "builds" => &[Category::BuildArtifacts, Category::PythonEnvs],
        "caches" => &[Category::PackageCaches, Category::AppCaches],
        "docker" => &[Category::Docker],
        "models" => &[Category::AiModels],
        "apps" => &[Category::UnusedApps],
        "downloads" => &[Category::Downloads],
        "trash" => &[Category::Trash],
        _ => &[],
    }
}

fn wanted(only: &[Category], category: Category) -> bool {
    only.is_empty() || only.contains(&category)
}

pub fn scan_all(ctx: &ScanContext) -> Vec<Item> {
    scan_only(ctx, &[])
}

pub fn scan_only(ctx: &ScanContext, only: &[Category]) -> Vec<Item> {
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
    use rayon::prelude::*;
    let mut items: Vec<Item> = SCANNERS
        .par_iter()
        .filter(|(name, _)| {
            only.is_empty() || scanner_categories(name).iter().any(|c| only.contains(c))
        })
        .flat_map(|(_, scan)| run_scanner(*scan, ctx))
        .filter(|item| wanted(only, item.category) && is_kept(item, &ctx.config.exclude))
        .collect();
    inherit_child_risk(&mut items);
    items.sort_by(compare_items);
    items
}

fn covering_path(action: &crate::model::Action) -> Option<&std::path::Path> {
    use crate::model::Action;
    match action {
        Action::DeletePath(path) | Action::MoveToTrash(path) => Some(path),
        _ => None,
    }
}

fn nested_path(action: &crate::model::Action) -> Option<&std::path::Path> {
    use crate::model::Action;
    match action {
        Action::PruneWorktrees { .. } => None,
        other => action_path(other),
    }
}

fn riskiest_child(items: &[Item], parent: &std::path::Path) -> Option<crate::model::Risk> {
    items
        .iter()
        .filter_map(|item| nested_path(&item.action).map(|path| (path, item.risk)))
        .filter(|(path, _)| *path != parent && path.starts_with(parent))
        .map(|(_, risk)| risk)
        .max()
}

pub fn inherit_child_risk(items: &mut [Item]) {
    use crate::model::Risk;
    const DELETE_WORD: &str = "delete";
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
            item.typed_confirmation = Some(DELETE_WORD.to_string());
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
    if item.bytes < MIN_ITEM_BYTES && !ignores_size_floor(&item.action) {
        return false;
    }
    let Some(path) = action_path(&item.action).map(folded) else {
        return true;
    };
    !exclude
        .iter()
        .map(|ex| folded(ex))
        .any(|ex| path.starts_with(&ex) || ex.starts_with(&path))
}

fn folded(path: &std::path::Path) -> PathBuf {
    PathBuf::from(path.to_string_lossy().to_lowercase())
}

fn ignores_size_floor(action: &crate::model::Action) -> bool {
    use crate::model::Action;
    const FSTRIM_ARG: &str = "fstrim";
    match action {
        Action::PruneWorktrees { .. } | Action::RemoveWorktree { .. } | Action::EmptyTrash => true,
        Action::RunCommand { args, .. } => args.iter().any(|arg| arg == FSTRIM_ARG),
        _ => false,
    }
}

fn action_path(action: &crate::model::Action) -> Option<&std::path::Path> {
    use crate::model::Action;
    match action {
        Action::DeletePath(path) | Action::MoveToTrash(path) => Some(path),
        Action::RemoveWorktree { worktree, .. } => Some(worktree),
        Action::PruneWorktrees { repo } => Some(repo),
        Action::RunCommand { .. } | Action::EmptyTrash => None,
    }
}
