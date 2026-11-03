use anyhow::{Context, bail};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

const CONFIG_RELATIVE_PATH: &str = ".config/diskdiet/config.toml";
const DEFAULT_DEV_ROOT_NAMES: [&str; 12] = [
    "Develop",
    "Developer",
    "dev",
    "code",
    "Code",
    "Projects",
    "projects",
    "src",
    "workspace",
    "repos",
    "GitHub",
    "git",
];
const DEFAULT_STALE_APP_DAYS: u64 = 90;
const DEFAULT_STALE_DOWNLOAD_DAYS: u64 = 90;
const DEFAULT_STALE_NODE_MODULES_DAYS: u64 = 30;
const DEFAULT_DOCKER_PRUNE_UNTIL_HOURS: u64 = 720;
const DEFAULT_LOW_SPACE_WARNING_GB: u64 = 100;
const TILDE: &str = "~";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub dev_roots: Vec<PathBuf>,
    pub exclude: Vec<PathBuf>,
    pub extra_cache_dirs: Vec<PathBuf>,
    pub stale_app_days: u64,
    pub stale_download_days: u64,
    pub stale_node_modules_days: u64,
    pub docker_prune_until_hours: u64,
    pub low_space_warning_gb: u64,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            dev_roots: Vec::new(),
            exclude: Vec::new(),
            extra_cache_dirs: Vec::new(),
            stale_app_days: DEFAULT_STALE_APP_DAYS,
            stale_download_days: DEFAULT_STALE_DOWNLOAD_DAYS,
            stale_node_modules_days: DEFAULT_STALE_NODE_MODULES_DAYS,
            docker_prune_until_hours: DEFAULT_DOCKER_PRUNE_UNTIL_HOURS,
            low_space_warning_gb: DEFAULT_LOW_SPACE_WARNING_GB,
        }
    }
}

pub fn default_path(home: &Path) -> PathBuf {
    home.join(CONFIG_RELATIVE_PATH)
}

// Case-insensitive volumes make "code" and "Code" the same folder, so identity is (dev, ino).
pub fn default_dev_roots(home: &Path) -> Vec<PathBuf> {
    let mut seen = HashSet::new();
    DEFAULT_DEV_ROOT_NAMES
        .iter()
        .map(|name| home.join(name))
        .filter(|path| {
            std::fs::metadata(path)
                .is_ok_and(|meta| meta.is_dir() && seen.insert((meta.dev(), meta.ino())))
        })
        .collect()
}

pub fn expand_tilde(path: &Path, home: &Path) -> PathBuf {
    match path.strip_prefix(TILDE) {
        Ok(rest) if rest.as_os_str().is_empty() => home.to_path_buf(),
        Ok(rest) => home.join(rest),
        Err(_) => path.to_path_buf(),
    }
}

fn expand_all(paths: &[PathBuf], home: &Path) -> Vec<PathBuf> {
    paths.iter().map(|path| expand_tilde(path, home)).collect()
}

fn parse_file(path: &Path) -> anyhow::Result<Config> {
    let text = std::fs::read_to_string(path)
        .with_context(|| format!("could not read config file {}", path.display()))?;
    toml::from_str(&text).with_context(|| format!("invalid config file {}", path.display()))
}

fn read_config(explicit: Option<&Path>, home: &Path) -> anyhow::Result<Config> {
    if let Some(path) = explicit {
        let path = expand_tilde(path, home);
        if !path.is_file() {
            bail!("config file not found: {}", path.display());
        }
        return parse_file(&path);
    }
    let path = default_path(home);
    if !path.is_file() {
        return Ok(Config::default());
    }
    parse_file(&path)
}

fn require_absolute(field: &str, paths: &[PathBuf]) -> anyhow::Result<()> {
    match paths.iter().find(|path| !path.is_absolute()) {
        Some(path) => bail!("{field} entry {} must start with / or ~", path.display()),
        None => Ok(()),
    }
}

pub fn load(explicit: Option<&Path>, home: &Path) -> anyhow::Result<Config> {
    let mut config = read_config(explicit, home)?;
    config.dev_roots = expand_all(&config.dev_roots, home);
    config.exclude = expand_all(&config.exclude, home);
    config.extra_cache_dirs = expand_all(&config.extra_cache_dirs, home);
    require_absolute("dev_roots", &config.dev_roots)?;
    require_absolute("exclude", &config.exclude)?;
    require_absolute("extra_cache_dirs", &config.extra_cache_dirs)?;
    if config.dev_roots.is_empty() {
        config.dev_roots = default_dev_roots(home);
    }
    Ok(config)
}
