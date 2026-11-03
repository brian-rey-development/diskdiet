use super::fixtures::write_file;
use anyhow::{Result, ensure};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

pub const GIT: &str = "git";

const GITIGNORE: &str = ".env\ntarget/\nnode_modules/\n";
const MAIN_BRANCH: &str = "main";

pub fn git(dir: &Path, args: &[&str]) -> Result<String> {
    let output = Command::new(GIT)
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

pub fn commit_file(repo: &Path, name: &str, contents: &str, message: &str) -> Result<()> {
    write_file(&repo.join(name), contents)?;
    git(repo, &["add", "-A"])?;
    git(repo, &["commit", "-m", message]).map(drop)
}

pub fn init_repo(repo: &Path) -> Result<()> {
    fs::create_dir_all(repo)?;
    git(repo, &["init", "-b", MAIN_BRANCH])?;
    write_file(&repo.join(".gitignore"), GITIGNORE)?;
    commit_file(repo, "README.md", "hello", "initial")
}

pub fn add_worktree(repo: &Path, path: &Path, branch: &str) -> Result<PathBuf> {
    let target = path.to_string_lossy();
    git(repo, &["worktree", "add", "-b", branch, &target])?;
    Ok(fs::canonicalize(path)?)
}

pub fn repo_with_worktree(root: &Path, branch: &str) -> Result<(PathBuf, PathBuf)> {
    let repo = root.join("repo");
    init_repo(&repo)?;
    let worktree = add_worktree(&repo, &root.join(format!("wt-{branch}")), branch)?;
    Ok((repo, worktree))
}

pub fn branch_exists(repo: &Path, name: &str) -> Result<bool> {
    let listing = git(repo, &["branch", "--list", name])?;
    Ok(!listing.trim().is_empty())
}
