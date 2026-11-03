use super::ScanContext;
use super::builds::tilde_path;
use crate::actions::is_safe_delete_target;
use crate::model::{Action, Category, Item, Risk};
use crate::size::disk_usage;
use crate::tools::run_with_deadline;
use rayon::prelude::*;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

const COMMAND_TIMEOUT: Duration = Duration::from_secs(20);
const MIN_APP_CACHE_BYTES: u64 = 50_000_000;
const APP_CACHES_DIR: &str = "Library/Caches";
const APPLE_CACHE_PREFIX: &str = "com.apple.";
const UPDATER_SUFFIX: &str = "-updater";
const REGENERABLE_NOTE: &str = "Re-downloaded on demand";

const EXCLUDED_NAMES: [&str; 8] = [
    "CloudKit",
    "GeoServices",
    "pip",
    "Yarn",
    "go-build",
    "Homebrew",
    "pnpm",
    "uv",
];

const SAFE_NAMES: [&str; 7] = [
    "ms-playwright",
    "electron",
    "electron-builder",
    "node-gyp",
    "Google",
    "Firefox",
    "Steam",
];

const RUNNING_OWNERS: [(&str, &str); 3] = [
    ("Google", "/Google Chrome.app/"),
    ("Firefox", "/Firefox.app/"),
    ("Steam", "/Steam.app/"),
];
const PATH_OWNERS: [(&str, &str); 1] = [("Library/Developer/Xcode/DerivedData", "/Xcode.app/")];
const OWNER_RUNNING_NOTE: &str = "Its app is running, quit it first";
const PS: &str = "/bin/ps";
const PS_ARGS: [&str; 2] = ["-axo", "comm="];

struct PathCache {
    label: &'static str,
    rel: &'static str,
}

const PATH_CACHES: [PathCache; 8] = [
    PathCache {
        label: "npm cache",
        rel: ".npm/_cacache",
    },
    PathCache {
        label: "Cargo registry cache",
        rel: ".cargo/registry/cache",
    },
    PathCache {
        label: "pip cache",
        rel: "Library/Caches/pip",
    },
    PathCache {
        label: "Bun install cache",
        rel: ".bun/install/cache",
    },
    PathCache {
        label: "Yarn cache",
        rel: "Library/Caches/Yarn",
    },
    PathCache {
        label: "Gradle caches",
        rel: ".gradle/caches",
    },
    PathCache {
        label: "Go build cache",
        rel: "Library/Caches/go-build",
    },
    PathCache {
        label: "Xcode DerivedData",
        rel: "Library/Developer/Xcode/DerivedData",
    },
];

struct CommandCache {
    tool: &'static str,
    label: &'static str,
    locate: &'static [&'static str],
    clean: &'static [&'static str],
}

const COMMAND_CACHES: [CommandCache; 3] = [
    CommandCache {
        tool: "uv",
        label: "uv cache",
        locate: &["cache", "dir"],
        clean: &["cache", "prune"],
    },
    CommandCache {
        tool: "pnpm",
        label: "pnpm store",
        locate: &["store", "path"],
        clean: &["store", "prune"],
    },
    CommandCache {
        tool: "brew",
        label: "Homebrew downloads",
        locate: &["--cache"],
        clean: &["cleanup", "--prune=all"],
    },
];

pub fn scan(ctx: &ScanContext) -> Vec<Item> {
    let processes = run_stdout(Path::new(PS), &PS_ARGS).unwrap_or_default();
    let mut items = package_caches(ctx, &processes);
    items.extend(app_caches(ctx, &processes));
    items
}

fn has_known_owner(item: &Item) -> Option<&Path> {
    let Action::DeletePath(path) = &item.action else {
        return None;
    };
    let named = RUNNING_OWNERS.iter().any(|(cache, _)| *cache == item.label);
    let located = PATH_OWNERS.iter().any(|(rel, _)| path.ends_with(rel));
    (named || located).then_some(path.as_path())
}

pub fn owner_is_running_now(item: &Item) -> bool {
    let Some(path) = has_known_owner(item) else {
        return false;
    };
    let processes = run_stdout(Path::new(PS), &PS_ARGS).unwrap_or_default();
    owner_is_running(&item.label, &processes) || path_owner_is_running(path, &processes)
}

pub fn path_owner_is_running(path: &Path, processes: &str) -> bool {
    PATH_OWNERS
        .iter()
        .any(|(rel, marker)| path.ends_with(rel) && processes.contains(marker))
}

pub fn app_cache_risk(name: &str) -> Option<Risk> {
    if name.starts_with(APPLE_CACHE_PREFIX) || EXCLUDED_NAMES.contains(&name) {
        return None;
    }
    let regenerable = SAFE_NAMES.contains(&name) || name.ends_with(UPDATER_SUFFIX);
    Some(if regenerable {
        Risk::Safe
    } else {
        Risk::Review
    })
}

pub fn owner_is_running(name: &str, processes: &str) -> bool {
    RUNNING_OWNERS
        .iter()
        .any(|(cache, marker)| *cache == name && processes.contains(marker))
}

pub fn app_cache_risk_now(name: &str, processes: &str) -> Option<Risk> {
    match app_cache_risk(name)? {
        Risk::Safe if owner_is_running(name, processes) => Some(Risk::Review),
        risk => Some(risk),
    }
}

pub(crate) fn make_item(
    category: Category,
    label: String,
    detail: String,
    bytes: u64,
    risk: Risk,
    action: Action,
) -> Item {
    Item {
        category,
        label,
        detail,
        bytes,
        risk,
        action,
        preselected: risk == Risk::Safe,
        typed_confirmation: None,
    }
}

pub(crate) fn run_stdout(program: &Path, args: &[&str]) -> Option<String> {
    let mut command = Command::new(program);
    command.args(args);
    let (status, stdout) = run_with_deadline(&mut command, COMMAND_TIMEOUT)?;
    let text = String::from_utf8_lossy(&stdout).trim().to_string();
    status.success().then_some(text)
}

pub(crate) fn measured(path: &Path) -> Option<u64> {
    if !path.exists() {
        return None;
    }
    let bytes = disk_usage(std::slice::from_ref(&path.to_path_buf()));
    (bytes > 0).then_some(bytes)
}

fn package_caches(ctx: &ScanContext, processes: &str) -> Vec<Item> {
    let commands = COMMAND_CACHES
        .par_iter()
        .filter_map(|spec| command_cache_item(ctx, spec));
    let paths = PATH_CACHES.par_iter().filter_map(|spec| {
        let path = ctx.home.join(spec.rel);
        let risk = path_cache_risk(&path, processes);
        path_cache_item(ctx, path, spec.label.to_string(), risk)
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
    if path_owner_is_running(path, processes) {
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
    let detail = format!("{note} · {}", tilde_path(&path, &ctx.home));
    let action = Action::DeletePath(path);
    Some(make_item(
        Category::PackageCaches,
        label,
        detail,
        bytes,
        risk,
        action,
    ))
}

fn command_cache_item(ctx: &ScanContext, spec: &CommandCache) -> Option<Item> {
    let tool = ctx.tools.get(spec.tool)?;
    let path = located_path(tool, spec)?;
    let bytes = measured(&path)?;
    let location = tilde_path(&path, &ctx.home);
    let detail = format!(
        "`{} {}` removes only unused entries, so up to this size · {location}",
        spec.tool,
        spec.clean.join(" ")
    );
    let label = spec.label.to_string();
    let action = clean_action(tool, spec);
    Some(make_item(
        Category::PackageCaches,
        label,
        detail,
        bytes,
        Risk::Safe,
        action,
    ))
}

fn located_path(tool: &Path, spec: &CommandCache) -> Option<PathBuf> {
    let path = PathBuf::from(run_stdout(tool, spec.locate)?);
    path.is_absolute().then_some(path)
}

fn clean_action(tool: &Path, spec: &CommandCache) -> Action {
    Action::RunCommand {
        program: tool.to_path_buf(),
        args: spec.clean.iter().map(ToString::to_string).collect(),
    }
}

fn app_caches(ctx: &ScanContext, processes: &str) -> Vec<Item> {
    let Ok(entries) = fs::read_dir(ctx.home.join(APP_CACHES_DIR)) else {
        return Vec::new();
    };
    let candidates: Vec<(PathBuf, Risk)> = entries
        .flatten()
        .filter_map(|e| candidate(&e, processes))
        .collect();
    candidates
        .into_par_iter()
        .filter_map(|(path, risk)| app_cache_item(ctx, path, risk))
        .collect()
}

fn candidate(entry: &fs::DirEntry, processes: &str) -> Option<(PathBuf, Risk)> {
    if !entry.file_type().ok()?.is_dir() {
        return None;
    }
    let name = entry.file_name().to_string_lossy().into_owned();
    Some((entry.path(), app_cache_risk_now(&name, processes)?))
}

fn app_cache_item(ctx: &ScanContext, path: PathBuf, risk: Risk) -> Option<Item> {
    let bytes = measured(&path).filter(|b| *b >= MIN_APP_CACHE_BYTES)?;
    let label = path.file_name()?.to_string_lossy().into_owned();
    let note = match risk {
        Risk::Safe => "Regenerable",
        _ if app_cache_risk(&label) == Some(Risk::Safe) => OWNER_RUNNING_NOTE,
        _ => "Unknown app cache, review before deleting",
    };
    let detail = format!("{note} · {}", tilde_path(&path, &ctx.home));
    let action = Action::DeletePath(path);
    Some(make_item(
        Category::AppCaches,
        label,
        detail,
        bytes,
        risk,
        action,
    ))
}
