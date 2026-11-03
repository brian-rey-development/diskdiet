use crate::support::SECS_PER_DAY;
use crate::support::STALE_DAYS;
use crate::support::fixtures::{temp_root, test_ctx, write_file};
use crate::support::git::{add_worktree, git, init_repo};
use anyhow::{Result, anyhow, ensure};
use diskdiet::domain::model::{Action, Category, Item, Risk};
use diskdiet::features::scan::builds::{classify, scan};
use std::fs::{self, OpenOptions};
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

const CACHEDIR_TAG: &str = "Signature: 8a477f597d28d172789f06886806bc55";

fn make_dir(path: &Path) -> Result<PathBuf> {
    fs::create_dir_all(path)?;
    Ok(path.to_path_buf())
}

fn payload(dir: &Path) -> Result<()> {
    write_file(&dir.join("debug/artifact.bin"), "0123456789")
}

fn age_file(path: &Path, days: u64) -> Result<()> {
    let file = OpenOptions::new().write(true).open(path)?;
    let old = SystemTime::now() - Duration::from_secs(days * SECS_PER_DAY);
    Ok(file.set_modified(old)?)
}

fn scan_root(root: &Path) -> Vec<Item> {
    scan(&test_ctx(root, vec![root.to_path_buf()]))
}

fn only_item(items: Vec<Item>) -> Result<Item> {
    let mut iter = items.into_iter();
    let first = iter.next().ok_or_else(|| anyhow!("no item produced"))?;
    ensure!(iter.next().is_none(), "more than one item produced");
    Ok(first)
}

#[test]
fn cachedir_tag_without_manifest_is_not_a_build_dir() -> Result<()> {
    let (_tmp, root) = temp_root()?;
    for name in ["target", "models", "cache", ".next"] {
        let dir = make_dir(&root.join("loose").join(name))?;
        write_file(&dir.join("CACHEDIR.TAG"), CACHEDIR_TAG)?;
        assert_eq!(classify(&dir), None, "{name} must not be a build dir");
    }
    assert_eq!(scan_root(&root), [] as [diskdiet::domain::model::Item; 0]);
    Ok(())
}

#[test]
fn target_needs_a_sibling_cargo_manifest() -> Result<()> {
    let (_tmp, root) = temp_root()?;
    let bare = make_dir(&root.join("bare/target"))?;
    assert_eq!(classify(&bare), None);
    write_file(&root.join("crate/Cargo.toml"), "[package]")?;
    let target = make_dir(&root.join("crate/target"))?;
    let expected = Some((Category::BuildArtifacts, Risk::Safe));
    assert_eq!(classify(&target), expected);
    Ok(())
}

#[test]
fn next_and_turbo_need_a_package_manifest() -> Result<()> {
    let (_tmp, root) = temp_root()?;
    assert_eq!(classify(&make_dir(&root.join("a/.next"))?), None);
    assert_eq!(classify(&make_dir(&root.join("a/.turbo"))?), None);
    write_file(&root.join("web/package.json"), "{}")?;
    let expected = Some((Category::BuildArtifacts, Risk::Safe));
    assert_eq!(classify(&make_dir(&root.join("web/.next"))?), expected);
    assert_eq!(classify(&make_dir(&root.join("web/.turbo"))?), expected);
    write_file(&root.join("mono/turbo.json"), "{}")?;
    assert_eq!(classify(&make_dir(&root.join("mono/.turbo"))?), expected);
    Ok(())
}

#[test]
fn venv_is_python_envs_review_and_never_preselected() -> Result<()> {
    let (_tmp, root) = temp_root()?;
    let venv = make_dir(&root.join("app/.venv"))?;
    write_file(&venv.join("pyvenv.cfg"), "home = /usr/bin")?;
    write_file(&venv.join("CACHEDIR.TAG"), CACHEDIR_TAG)?;
    assert_eq!(classify(&venv), Some((Category::PythonEnvs, Risk::Review)));
    let item = only_item(scan_root(&root))?;
    assert_eq!(item.category, Category::PythonEnvs);
    assert!(!item.preselected);
    assert!(item.detail.contains("uv sync"));
    Ok(())
}

#[test]
fn a_dot_venv_folder_without_pyvenv_cfg_is_not_offered() -> Result<()> {
    let (_tmp, root) = temp_root()?;
    let folder = make_dir(&root.join("app/.venv"))?;
    write_file(&folder.join("notes.txt"), "not a virtualenv")?;
    assert_eq!(classify(&folder), None);
    Ok(())
}

#[test]
fn target_with_manifest_is_safe_and_preselected_in_scan() -> Result<()> {
    let (_tmp, root) = temp_root()?;
    write_file(&root.join("crate/Cargo.toml"), "[package]")?;
    payload(&root.join("crate/target"))?;
    let item = only_item(scan_root(&root))?;
    assert_eq!(item.category, Category::BuildArtifacts);
    assert_eq!((item.risk, item.preselected), (Risk::Safe, true));
    assert_eq!(item.action, Action::DeletePath(root.join("crate/target")));
    assert!(item.typed_confirmation.is_none());
    Ok(())
}

#[test]
fn node_modules_is_offered_only_when_the_project_is_stale() -> Result<()> {
    let (_tmp, root) = temp_root()?;
    for project in ["fresh", "stale"] {
        write_file(&root.join(project).join("package.json"), "{}")?;
        write_file(&root.join(project).join("node_modules/dep/index.js"), "x")?;
    }
    age_file(&root.join("stale/package.json"), STALE_DAYS)?;
    let item = only_item(scan_root(&root))?;
    assert_eq!(
        item.action,
        Action::DeletePath(root.join("stale/node_modules"))
    );
    assert_eq!((item.risk, item.preselected), (Risk::Review, false));
    Ok(())
}

#[test]
fn nested_matches_are_not_double_counted() -> Result<()> {
    let (_tmp, root) = temp_root()?;
    write_file(&root.join("outer/Cargo.toml"), "[package]")?;
    write_file(&root.join("outer/target/vendored/Cargo.toml"), "[package]")?;
    payload(&root.join("outer/target/vendored/target"))?;
    let item = only_item(scan_root(&root))?;
    assert_eq!(item.action, Action::DeletePath(root.join("outer/target")));
    Ok(())
}

#[test]
fn symlinked_target_is_never_a_build_dir() -> Result<()> {
    let (_tmp, root) = temp_root()?;
    write_file(&root.join("crate/Cargo.toml"), "[package]")?;
    let real = make_dir(&root.join("elsewhere"))?;
    symlink(&real, root.join("crate/target"))?;
    assert_eq!(classify(&root.join("crate/target")), None);
    assert_eq!(scan_root(&root), [] as [diskdiet::domain::model::Item; 0]);
    Ok(())
}

#[test]
fn linked_worktrees_are_left_to_the_worktree_scanner() -> Result<()> {
    let (_tmp, root) = temp_root()?;
    let repo = root.join("repo");
    init_repo(&repo)?;
    write_file(&repo.join("Cargo.toml"), "[package]")?;
    git(&repo, &["add", "-A"])?;
    git(&repo, &["commit", "-m", "manifest"])?;
    let worktree = add_worktree(&repo, &repo.join(".claude/worktrees/agent"), "agent")?;
    payload(&worktree.join("target"))?;
    assert_eq!(scan_root(&root), [] as [diskdiet::domain::model::Item; 0]);
    Ok(())
}
