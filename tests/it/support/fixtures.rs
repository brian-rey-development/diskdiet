use anyhow::Result;
use diskdiet::config::Config;
use diskdiet::model::{Action, Category, Item, Risk};
use diskdiet::scan::ScanContext;
use diskdiet::tools::Tools;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

const FIXTURE_BYTES: u64 = 1024;

pub struct RestorePermissions(pub PathBuf);

impl Drop for RestorePermissions {
    fn drop(&mut self) {
        let _ = set_mode(&self.0, super::RWX_MODE);
    }
}

pub fn set_mode(path: &Path, mode: u32) -> Result<()> {
    Ok(fs::set_permissions(path, fs::Permissions::from_mode(mode))?)
}

pub fn temp_root() -> Result<(TempDir, PathBuf)> {
    let dir = tempfile::tempdir()?;
    let root = fs::canonicalize(dir.path())?;
    Ok((dir, root))
}

pub fn write_file(path: &Path, contents: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    Ok(fs::write(path, contents)?)
}

pub fn test_config(dev_roots: Vec<PathBuf>) -> Config {
    Config {
        dev_roots,
        ..Config::default()
    }
}

pub fn test_ctx(home: &Path, dev_roots: Vec<PathBuf>) -> ScanContext {
    ScanContext {
        home: home.to_path_buf(),
        config: test_config(dev_roots),
        tools: Tools::default(),
    }
}

pub fn fixture_item(action: Action) -> Item {
    Item::new(
        Category::BuildArtifacts,
        "fixture",
        "fixture detail",
        FIXTURE_BYTES,
        Risk::Safe,
        action,
    )
}

pub fn sized_item(category: Category, risk: Risk, bytes: u64, action: Action) -> Item {
    let label = format!("{category:?}-{risk:?}-{bytes}");
    Item::new(category, label, "", bytes, risk, action)
}

pub fn delete_item(path: &Path, risk: Risk, bytes: u64) -> Item {
    let action = Action::DeletePath(path.to_path_buf());
    sized_item(Category::BuildArtifacts, risk, bytes, action)
}
