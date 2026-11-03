use anyhow::Result;
use diskdiet::platform::{self, IS_MACOS, is_mount_point, volume_is_mounted};
use std::path::Path;
use tempfile::tempdir;

#[test]
fn drives_under_linux_mount_points_count_as_unplugged_when_missing() {
    assert!(!volume_is_mounted(Path::new(
        "/media/me/diskdiet-missing/wt"
    )));
    assert!(!volume_is_mounted(Path::new("/mnt/diskdiet-missing/wt")));
    assert!(!volume_is_mounted(Path::new(
        "/Volumes/diskdiet-missing/wt"
    )));
    assert!(volume_is_mounted(Path::new("/home/me/wt")));
}

#[test]
fn platform_paths_follow_each_system_convention() {
    let home = Path::new("/home/me");
    let (log, trash) = (platform::log_path(home), platform::trash_dir(home));
    if IS_MACOS {
        assert_eq!(log, home.join("Library/Logs/diskdiet.log"));
        assert_eq!(trash, home.join(".Trash"));
        return;
    }
    assert!(log.ends_with("diskdiet/diskdiet.log"));
    assert!(trash.ends_with("Trash"));
}

#[test]
fn a_plain_folder_is_not_a_mount_point() -> Result<()> {
    let temp = tempdir()?;
    let folder = temp.path().join("not-a-mount");
    std::fs::create_dir(&folder)?;
    assert!(!is_mount_point(&folder));
    assert!(!is_mount_point(&temp.path().join("missing")));
    assert!(!is_mount_point(Path::new("/")));
    Ok(())
}

#[test]
fn a_separate_filesystem_is_a_mount_point() {
    assert!(is_mount_point(Path::new("/dev")));
}
