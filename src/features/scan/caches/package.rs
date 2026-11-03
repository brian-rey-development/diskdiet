use super::owners::{OWNER_RUNNING_NOTE, path_owner_in};
use crate::domain::model::{Action, Category, Item, Risk};
use crate::domain::safety::is_safe_delete_target;
use crate::features::scan::ScanContext;
use crate::features::scan::support::{located_detail, measured};
use crate::infra::sys::platform::IS_MACOS;
use crate::infra::sys::process::{DEFAULT_TIMEOUT, run_stdout};
use crate::infra::sys::tools;
use crate::infra::util::fmt::tilde_path;
use rayon::prelude::*;
use std::path::{Path, PathBuf};

const REGENERABLE_NOTE: &str = "Re-downloaded on demand";

struct PathCache {
    label: &'static str,
    mac: Option<&'static str>,
    linux: Option<&'static str>,
}

impl PathCache {
    const fn everywhere(label: &'static str, relative_path: &'static str) -> Self {
        Self {
            label,
            mac: Some(relative_path),
            linux: Some(relative_path),
        }
    }

    const fn per_platform(label: &'static str, mac: &'static str, linux: &'static str) -> Self {
        Self {
            label,
            mac: Some(mac),
            linux: Some(linux),
        }
    }

    const fn macos_only(label: &'static str, relative_path: &'static str) -> Self {
        Self {
            label,
            mac: Some(relative_path),
            linux: None,
        }
    }

    fn relative_path(&self) -> Option<&'static str> {
        if IS_MACOS { self.mac } else { self.linux }
    }
}

const PATH_CACHES: [PathCache; 8] = [
    PathCache::everywhere("npm cache", ".npm/_cacache"),
    PathCache::everywhere("Cargo registry cache", ".cargo/registry/cache"),
    PathCache::per_platform("pip cache", "Library/Caches/pip", ".cache/pip"),
    PathCache::everywhere("Bun install cache", ".bun/install/cache"),
    PathCache::per_platform("Yarn cache", "Library/Caches/Yarn", ".cache/yarn"),
    PathCache::everywhere("Gradle caches", ".gradle/caches"),
    PathCache::per_platform(
        "Go build cache",
        "Library/Caches/go-build",
        ".cache/go-build",
    ),
    PathCache::macos_only("Xcode DerivedData", "Library/Developer/Xcode/DerivedData"),
];

struct CommandCache {
    tool: &'static str,
    label: &'static str,
    locate: &'static [&'static str],
    clean: &'static [&'static str],
}

const COMMAND_CACHES: [CommandCache; 3] = [
    CommandCache {
        tool: tools::UV,
        label: "uv cache",
        locate: &["cache", "dir"],
        clean: &["cache", "prune"],
    },
    CommandCache {
        tool: tools::PNPM,
        label: "pnpm store",
        locate: &["store", "path"],
        clean: &["store", "prune"],
    },
    CommandCache {
        tool: tools::BREW,
        label: "Homebrew downloads",
        locate: &["--cache"],
        clean: &["cleanup", "--prune=all"],
    },
];

pub(super) fn package_caches(ctx: &ScanContext, processes: &str) -> Vec<Item> {
    let commands = COMMAND_CACHES
        .par_iter()
        .filter_map(|spec| command_cache_item(ctx, spec));
    let paths = PATH_CACHES.par_iter().filter_map(|spec| {
        let path = ctx.home.join(spec.relative_path()?);
        let risk = path_cache_risk(&path, processes);
        path_cache_item(ctx, path, spec.label.to_owned(), risk)
    });
    let extras = ctx
        .config
        .extra_cache_dirs
        .par_iter()
        .filter(|dir| is_safe_delete_target(dir, Some(&ctx.home)))
        .filter_map(|dir| {
            let label = tilde_path(dir, &ctx.home);
            path_cache_item(ctx, dir.clone(), label, Risk::Safe)
        });
    commands.chain(paths).chain(extras).collect()
}

fn path_cache_risk(path: &Path, processes: &str) -> Risk {
    if path_owner_in(path, processes) {
        return Risk::Review;
    }
    Risk::Safe
}

fn path_cache_item(ctx: &ScanContext, path: PathBuf, label: String, risk: Risk) -> Option<Item> {
    let bytes = measured(&path)?;
    let note = match risk {
        Risk::Safe => REGENERABLE_NOTE,
        _ => OWNER_RUNNING_NOTE,
    };
    let detail = located_detail(note, &path, &ctx.home);
    Some(Item::new(
        Category::PackageCaches,
        label,
        detail,
        bytes,
        risk,
        Action::DeletePath(path),
    ))
}

fn command_cache_item(ctx: &ScanContext, spec: &CommandCache) -> Option<Item> {
    let tool = ctx.tools.get(spec.tool)?;
    let path = located_path(tool, spec)?;
    let bytes = measured(&path)?;
    let note = format!(
        "`{} {}` removes only unused entries, so up to this size",
        spec.tool,
        spec.clean.join(" ")
    );
    Some(Item::new(
        Category::PackageCaches,
        spec.label,
        located_detail(&note, &path, &ctx.home),
        bytes,
        Risk::Safe,
        clean_action(tool, spec),
    ))
}

fn located_path(tool: &Path, spec: &CommandCache) -> Option<PathBuf> {
    let path = PathBuf::from(run_stdout(tool, spec.locate, DEFAULT_TIMEOUT)?);
    path.is_absolute().then_some(path)
}

fn clean_action(tool: &Path, spec: &CommandCache) -> Action {
    Action::RunCommand {
        program: tool.to_path_buf(),
        args: spec.clean.iter().map(|arg| (*arg).to_owned()).collect(),
    }
}
