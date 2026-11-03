use crate::platform;
use crate::process::run_with_deadline;
use rayon::prelude::*;
use std::collections::HashMap;
use std::ffi::{OsStr, OsString};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;
use std::time::Duration;

pub(crate) const GIT: &str = "git";
pub(crate) const UV: &str = "uv";
pub(crate) const PNPM: &str = "pnpm";
pub(crate) const NPM: &str = "npm";
pub(crate) const BREW: &str = "brew";
pub(crate) const DOCKER: &str = "docker";
pub(crate) const COLIMA: &str = "colima";
pub(crate) const OLLAMA: &str = "ollama";
pub(crate) const CARGO: &str = "cargo";
pub(crate) const OSASCRIPT: &str = "osascript";
pub(crate) const OSASCRIPT_FALLBACK: &str = "/usr/bin/osascript";
pub(crate) const MDLS: &str = "mdls";

pub(crate) const KNOWN_TOOLS: [&str; 11] = [
    GIT, UV, PNPM, NPM, BREW, DOCKER, COLIMA, OLLAMA, CARGO, OSASCRIPT, MDLS,
];

const EXEC_BITS: u32 = 0o111;
const FALLBACK_SHELL: &str = "/bin/sh";
const SHELL_FLAGS: &str = "-lic";
const MISE: &str = "mise";
const MISE_FIXED_LOCATIONS: [&str; 2] = ["/opt/homebrew/bin/mise", "/usr/local/bin/mise"];
const MISE_HOME_LOCATION: &str = ".local/bin/mise";
const LOOKUP_TIMEOUT: Duration = Duration::from_secs(10);
const PATH_MARKER: &str = "__DISKDIET_PATH__=";
const PRINT_PATH: &str = "printf '\\n__DISKDIET_PATH__=%s\\n' \"$PATH\"";

type LoginShellPath = OnceLock<Option<OsString>>;

#[derive(Debug, Clone, Default)]
pub struct Tools {
    resolved: HashMap<String, PathBuf>,
}

impl Tools {
    pub fn discover() -> Self {
        let login_path = LoginShellPath::new();
        let resolved = KNOWN_TOOLS
            .par_iter()
            .filter_map(|name| {
                let path = locate_tool(name, &login_path)?;
                Some(((*name).to_owned(), path))
            })
            .collect();
        Self { resolved }
    }

    pub fn from_map(resolved: HashMap<String, PathBuf>) -> Self {
        Self { resolved }
    }

    pub fn get(&self, name: &str) -> Option<&Path> {
        self.resolved.get(name).map(PathBuf::as_path)
    }

    pub(crate) fn get_or<'a>(&'a self, name: &str, fallback: &'a str) -> &'a Path {
        self.get(name).unwrap_or_else(|| Path::new(fallback))
    }
}

fn locate_tool(name: &str, login_path: &LoginShellPath) -> Option<PathBuf> {
    resolve_on_path(name)
        .or_else(|| {
            let path = login_path.get_or_init(login_shell_path).as_deref()?;
            find_on_path(name, path)
        })
        .or_else(|| resolve_via_mise(name))
}

fn is_executable(path: &Path) -> bool {
    std::fs::metadata(path)
        .is_ok_and(|meta| meta.is_file() && meta.permissions().mode() & EXEC_BITS != 0)
}

pub fn find_on_path(name: &str, path_var: &OsStr) -> Option<PathBuf> {
    std::env::split_paths(path_var)
        .filter(|dir| dir.is_absolute())
        .map(|dir| dir.join(name))
        .find(|candidate| is_executable(candidate))
}

fn resolve_on_path(name: &str) -> Option<PathBuf> {
    let path_var = std::env::var_os("PATH")?;
    find_on_path(name, &path_var)
}

pub fn path_from_login_shell_output(output: &str) -> Option<OsString> {
    output
        .lines()
        .find_map(|line| line.trim().strip_prefix(PATH_MARKER))
        .map(OsString::from)
}

fn login_shell_path() -> Option<OsString> {
    let shell = std::env::var_os("SHELL").unwrap_or_else(|| FALLBACK_SHELL.into());
    let mut command = Command::new(shell);
    command.args([SHELL_FLAGS, PRINT_PATH]);
    let (_, stdout) = run_with_deadline(&mut command, LOOKUP_TIMEOUT)?;
    path_from_login_shell_output(&String::from_utf8_lossy(&stdout))
}

fn mise_binary() -> Option<PathBuf> {
    let mut candidates: Vec<PathBuf> = MISE_FIXED_LOCATIONS.iter().map(PathBuf::from).collect();
    if let Some(home) = platform::home_dir() {
        candidates.push(home.join(MISE_HOME_LOCATION));
    }
    resolve_on_path(MISE).or_else(|| candidates.into_iter().find(|path| is_executable(path)))
}

fn resolve_via_mise(name: &str) -> Option<PathBuf> {
    let mut command = Command::new(mise_binary()?);
    command.args(["which", name]);
    let (status, stdout) = run_with_deadline(&mut command, LOOKUP_TIMEOUT)?;
    let found = last_absolute_path(&String::from_utf8_lossy(&stdout))?;
    let named = found.file_name() == Some(OsStr::new(name));
    (status.success() && named && is_executable(&found)).then_some(found)
}

fn path_in_segment(segment: &str) -> Option<PathBuf> {
    let start = segment.find('/')?;
    Some(PathBuf::from(segment[start..].trim()))
}

pub fn last_absolute_path(output: &str) -> Option<PathBuf> {
    output
        .lines()
        .rev()
        .flat_map(|line| line.rsplit(char::is_control))
        .filter_map(path_in_segment)
        .find(|path| path.is_absolute() && path.exists())
}
