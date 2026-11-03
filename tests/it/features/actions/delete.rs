use crate::support::fixtures::{RestorePermissions, set_mode};
use crate::support::{READ_ONLY_DIR_MODE, RWX_MODE};
use anyhow::Result;
use diskdiet::domain::model::{Action, Outcome};
use diskdiet::features::actions::perform;
use diskdiet::infra::sys::tools::Tools;
use std::fs;
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

fn delete(path: &Path) -> Outcome {
    perform(&Action::DeletePath(path.to_path_buf()), &Tools::default())
}

#[test]
fn delete_path_removes_a_directory_tree_and_a_file() -> Result<()> {
    let temp = TempDir::new()?;
    let dir = temp.path().join("tree");
    fs::create_dir_all(dir.join("a/b"))?;
    fs::write(dir.join("a/b/file"), "data")?;
    let file = temp.path().join("single.txt");
    fs::write(&file, "data")?;
    assert_eq!(delete(&dir), Outcome::Done);
    assert_eq!(delete(&file), Outcome::Done);
    assert!(!dir.exists());
    assert!(!file.exists());
    Ok(())
}

#[test]
fn delete_path_does_not_follow_symlinks() -> Result<()> {
    let temp = TempDir::new()?;
    let target = temp.path().join("precious");
    fs::create_dir(&target)?;
    fs::write(target.join("keep"), "x")?;
    let link = temp.path().join("link");
    symlink(&target, &link)?;
    assert_eq!(delete(&link), Outcome::Done);
    assert!(!link.exists());
    assert!(target.join("keep").exists());
    Ok(())
}

#[test]
fn delete_path_on_missing_path_is_skipped() -> Result<()> {
    let temp = TempDir::new()?;
    let outcome = delete(&temp.path().join("nope"));
    assert!(matches!(outcome, Outcome::Skipped(_)));
    Ok(())
}

#[test]
fn delete_path_refuses_root_and_relative_paths() {
    for path in ["/", "/Users", "relative/dir"] {
        let outcome = delete(&PathBuf::from(path));
        assert!(
            matches!(outcome, Outcome::Failed(_)),
            "{path} must be refused"
        );
    }
}

#[test]
fn delete_path_in_read_only_parent_needs_full_disk_access() -> Result<()> {
    let temp = TempDir::new()?;
    let locked = temp.path().join("locked");
    fs::create_dir(&locked)?;
    let victim = locked.join("victim");
    fs::write(&victim, "x")?;
    let _restore = RestorePermissions(locked.clone());
    set_mode(&locked, READ_ONLY_DIR_MODE)?;
    let outcome = delete(&victim);
    set_mode(&locked, RWX_MODE)?;
    assert_eq!(outcome, Outcome::NeedsFullDiskAccess);
    assert!(victim.exists());
    Ok(())
}
