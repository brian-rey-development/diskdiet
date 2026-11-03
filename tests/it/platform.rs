use diskdiet::platform::{self, IS_MACOS, volume_is_mounted};
use std::path::Path;

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
