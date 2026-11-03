#![allow(dead_code)]

use anyhow::{Result, ensure};
use diskdiet::config::Config;
use diskdiet::scan::ScanContext;
use diskdiet::tools::Tools;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::TempDir;

const GITIGNORE: &str = ".env\ntarget/\nnode_modules/\n";

pub fn temp_root() -> Result<(TempDir, PathBuf)> {
    let dir = tempfile::tempdir()?;
    let root = fs::canonicalize(dir.path())?;
    Ok((dir, root))
}

pub fn git(dir: &Path, args: &[&str]) -> Result<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(["-c", "user.email=test@example.com", "-c", "user.name=Test"])
        .args(["-c", "commit.gpgsign=false"])
        .args(args)
        .output()?;
    let stderr = String::from_utf8_lossy(&output.stderr);
    ensure!(output.status.success(), "git {args:?} failed: {stderr}");
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

pub fn write_file(path: &Path, contents: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    Ok(fs::write(path, contents)?)
}

pub fn commit_file(repo: &Path, name: &str, contents: &str, message: &str) -> Result<()> {
    write_file(&repo.join(name), contents)?;
    git(repo, &["add", "-A"])?;
    git(repo, &["commit", "-m", message]).map(drop)
}

pub fn init_repo(repo: &Path) -> Result<()> {
    fs::create_dir_all(repo)?;
    git(repo, &["init", "-b", "main"])?;
    write_file(&repo.join(".gitignore"), GITIGNORE)?;
    commit_file(repo, "README.md", "hello", "initial")
}

pub fn add_worktree(repo: &Path, path: &Path, branch: &str) -> Result<PathBuf> {
    let target = path.to_string_lossy();
    git(repo, &["worktree", "add", "-b", branch, &target])?;
    Ok(fs::canonicalize(path)?)
}

pub fn test_config(dev_roots: Vec<PathBuf>) -> Config {
    Config {
        dev_roots,
        exclude: Vec::new(),
        extra_cache_dirs: Vec::new(),
        stale_app_days: 90,
        stale_download_days: 90,
        stale_node_modules_days: 30,
        docker_prune_until_hours: 720,
        low_space_warning_gb: 100,
    }
}

pub fn test_ctx(home: &Path, dev_roots: Vec<PathBuf>) -> ScanContext {
    ScanContext {
        home: home.to_path_buf(),
        config: test_config(dev_roots),
        tools: Tools::default(),
    }
}
