use std::path::{Path, PathBuf};

pub const IS_MACOS: bool = cfg!(target_os = "macos");

#[cfg(target_os = "macos")]
mod paths {
    pub const LOG: &str = "Library/Logs/diskdiet.log";
    pub const TRASH: &str = ".Trash";
    pub const APP_CACHES: &str = "Library/Caches";
    pub const MACHINE: &str = "Mac";
}

#[cfg(not(target_os = "macos"))]
mod paths {
    pub const LOG: &str = ".local/state/diskdiet/diskdiet.log";
    pub const TRASH: &str = ".local/share/Trash";
    pub const APP_CACHES: &str = ".cache";
    pub const MACHINE: &str = "Linux machine";
}

pub const MACHINE: &str = paths::MACHINE;
pub const NO_HOME: &str = "HOME is not set";
const HOME_VAR: &str = "HOME";
pub(crate) const MOUNT_ROOTS: [(&str, usize); 4] = [
    ("/Volumes", 1),
    ("/run/media", 2),
    ("/media", 2),
    ("/mnt", 1),
];
const XDG_DATA_HOME: &str = "XDG_DATA_HOME";
const XDG_STATE_HOME: &str = "XDG_STATE_HOME";
const XDG_TRASH: &str = "Trash";
const XDG_LOG: &str = "diskdiet/diskdiet.log";

fn xdg_dir(var: &str) -> Option<PathBuf> {
    if IS_MACOS {
        return None;
    }
    std::env::var_os(var)
        .map(PathBuf::from)
        .filter(|dir| dir.is_absolute())
}

pub fn log_path(home: &Path) -> PathBuf {
    xdg_dir(XDG_STATE_HOME).map_or_else(|| home.join(paths::LOG), |dir| dir.join(XDG_LOG))
}

pub fn trash_dir(home: &Path) -> PathBuf {
    xdg_dir(XDG_DATA_HOME).map_or_else(|| home.join(paths::TRASH), |dir| dir.join(XDG_TRASH))
}

pub(crate) fn app_caches_dir(home: &Path) -> PathBuf {
    home.join(paths::APP_CACHES)
}

pub fn home_dir() -> Option<PathBuf> {
    std::env::var_os(HOME_VAR).map(PathBuf::from)
}

pub fn is_mount_point(path: &Path) -> bool {
    let Some(parent) = path.parent() else {
        return false;
    };
    match (rustix::fs::stat(path), rustix::fs::stat(parent)) {
        (Ok(own), Ok(outer)) => own.st_dev != outer.st_dev,
        _ => false,
    }
}

pub fn volume_is_mounted(path: &Path) -> bool {
    for (root, depth) in MOUNT_ROOTS {
        let Ok(rest) = path.strip_prefix(root) else {
            continue;
        };
        let parts: Vec<_> = rest.components().take(depth).collect();
        if parts.len() < depth {
            return true;
        }
        let mount = parts
            .iter()
            .fold(PathBuf::from(root), |acc, part| acc.join(part));
        return is_mount_point(&mount);
    }
    true
}
