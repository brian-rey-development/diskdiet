use crate::support::fixtures::{temp_root, write_file};
use crate::support::git::{GIT, commit_file, git, init_repo};
use anyhow::{Result, anyhow};
use diskdiet::git::ignored::is_build_output;
use diskdiet::git::status::local_changes;
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

const IGNORE_BUILD_AND_DIST: &str = "build/\ndist/\n";

fn manifest_root() -> Result<(TempDir, PathBuf)> {
    let (tmp, root) = temp_root()?;
    write_file(&root.join("Cargo.toml"), "")?;
    Ok((tmp, root))
}

fn risky_ignored(worktree: &Path) -> Result<u32> {
    let counts = local_changes(Path::new(GIT), worktree).ok_or_else(|| anyhow!("status failed"))?;
    Ok(counts.risky_ignored)
}

fn repo_ignoring_build_dirs(root: &Path) -> Result<PathBuf> {
    let repo = root.join("repo");
    init_repo(&repo)?;
    commit_file(&repo, ".gitignore", IGNORE_BUILD_AND_DIST, "ignore")?;
    Ok(repo)
}

#[test]
fn build_output_is_recognised_by_any_path_component() -> Result<()> {
    let (_tmp, root) = manifest_root()?;
    for known in [
        "target/",
        "apps/web/node_modules/",
        ".next/",
        ".venv/",
        "x/__pycache__/",
        "apps/web/next-env.d.ts",
        "apps/web/tsconfig.tsbuildinfo",
        ".husky/_/",
        ".husky/_/pre-commit",
        "coverage/",
        "apps/web/.tanstack/",
        "app/src-tauri/gen/",
    ] {
        assert!(is_build_output(known, &root), "{known}");
    }
    Ok(())
}

#[test]
fn personal_files_are_never_build_output() -> Result<()> {
    let (_tmp, root) = manifest_root()?;
    for personal in [
        ".env",
        "config/.env.local",
        "secrets/key.pem",
        "targets/a",
        "notes.md",
        ".husky/pre-commit",
        ".vercel/project.json",
        "src/gen/schema.rs",
        "pkg/dist/a.js",
        "deploy/build/signing.pem",
        "target",
    ] {
        assert!(!is_build_output(personal, &root), "{personal}");
    }
    Ok(())
}

#[test]
fn ambiguous_directories_need_a_manifest_in_their_parent() -> Result<()> {
    let (_tmp, root) = manifest_root()?;
    write_file(&root.join("packages/web/package.json"), "{}")?;
    fs::create_dir_all(root.join("deploy"))?;
    assert!(is_build_output("packages/web/dist/", &root));
    assert!(is_build_output("build/", &root));
    assert!(!is_build_output("deploy/build/", &root));
    assert!(!is_build_output("packages/dist/", &root));
    assert!(is_build_output("deploy/node_modules/", &root));
    Ok(())
}

#[test]
fn an_ignored_build_folder_without_a_manifest_counts_as_risky() -> Result<()> {
    let (_tmp, root) = temp_root()?;
    let repo = repo_ignoring_build_dirs(&root)?;
    write_file(&repo.join("deploy/build/signing.pem"), "secret")?;
    let status = git(&repo, &["status", "--porcelain=v1", "--ignored=matching"])?;
    assert!(status.contains("!! deploy/build/"), "{status}");
    assert_eq!(risky_ignored(&repo)?, 1);
    Ok(())
}

#[test]
fn an_ignored_build_folder_next_to_a_manifest_is_excused() -> Result<()> {
    let (_tmp, root) = temp_root()?;
    let repo = repo_ignoring_build_dirs(&root)?;
    commit_file(&repo, "package.json", "{}", "manifest")?;
    commit_file(&repo, "packages/web/package.json", "{}", "nested manifest")?;
    write_file(&repo.join("build/out.js"), "x")?;
    write_file(&repo.join("packages/web/dist/out.js"), "x")?;
    assert_eq!(risky_ignored(&repo)?, 0);
    Ok(())
}
