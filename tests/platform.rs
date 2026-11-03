use anyhow::{Result, anyhow};
use diskdiet::freedesktop_trash::{empty, encode_path, info_text, move_to};
use diskdiet::model::Risk;
use diskdiet::platform::{self, IS_MACOS, volume_is_mounted};
use diskdiet::scan::caches::process_matches;
use diskdiet::scan::downloads::{classify_download, is_installer};
use std::fs;
use std::path::Path;
use tempfile::tempdir;

const DELETED_AT: &str = "2026-11-03T10:00:00";
const STALE_DAYS: u64 = 90;

#[test]
fn trash_info_follows_the_freedesktop_format() {
    let text = info_text(Path::new("/home/me/My Notes/ñ.txt"), DELETED_AT);
    assert_eq!(
        text,
        "[Trash Info]\nPath=/home/me/My%20Notes/%C3%B1.txt\nDeletionDate=2026-11-03T10:00:00\n"
    );
    assert_eq!(encode_path(Path::new("/a-b_c.d~e")), "/a-b_c.d~e");
}

#[test]
fn moving_to_the_trash_keeps_the_file_and_records_where_it_came_from() -> Result<()> {
    let home = tempdir()?;
    let trash = home.path().join("Trash");
    let file = home.path().join("report.pdf");
    fs::write(&file, "pdf")?;
    move_to(&trash, &file, DELETED_AT)?;
    assert!(!file.exists());
    assert_eq!(fs::read_to_string(trash.join("files/report.pdf"))?, "pdf");
    let info = fs::read_to_string(trash.join("info/report.pdf.trashinfo"))?;
    assert!(info.contains(&format!("Path={}", encode_path(&file))));
    Ok(())
}

#[test]
fn a_second_file_with_the_same_name_gets_a_new_trash_name() -> Result<()> {
    let home = tempdir()?;
    let trash = home.path().join("Trash");
    for folder in ["a", "b"] {
        let file = home.path().join(folder).join("setup.deb");
        fs::create_dir_all(file.parent().ok_or_else(|| anyhow!("parent"))?)?;
        fs::write(&file, folder)?;
        move_to(&trash, &file, DELETED_AT)?;
    }
    assert_eq!(fs::read_to_string(trash.join("files/setup.deb"))?, "a");
    assert_eq!(fs::read_to_string(trash.join("files/setup.deb.2"))?, "b");
    assert!(trash.join("info/setup.deb.2.trashinfo").exists());
    Ok(())
}

#[test]
fn emptying_the_trash_clears_files_info_and_expunged() -> Result<()> {
    let home = tempdir()?;
    let trash = home.path().join("Trash");
    let folder = home.path().join("old-project");
    fs::create_dir_all(folder.join("src"))?;
    fs::write(folder.join("src/main.rs"), "fn main() {}")?;
    move_to(&trash, &folder, DELETED_AT)?;
    fs::create_dir_all(trash.join("expunged/leftover"))?;
    fs::write(trash.join("directorysizes"), "cache")?;
    empty(&trash)?;
    for dir in ["files", "info", "expunged"] {
        assert_eq!(fs::read_dir(trash.join(dir))?.count(), 0, "{dir}");
    }
    assert!(!trash.join("directorysizes").exists());
    Ok(())
}

#[test]
fn emptying_a_trash_that_does_not_exist_is_fine() -> Result<()> {
    let home = tempdir()?;
    empty(&home.path().join("never-created"))?;
    Ok(())
}

#[test]
fn linux_process_names_match_whole_names_only() {
    let processes = "systemd\n  chrome\n/usr/lib/firefox/firefox\nchrome_crashpad\n";
    assert!(process_matches(processes, "chrome"));
    assert!(process_matches(processes, "firefox"));
    assert!(!process_matches("chrome_crashpad\n", "chrome"));
    assert!(!process_matches(processes, "steam"));
}

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
fn linux_packages_count_as_installers() {
    assert!(is_installer("tool_1.2_amd64.deb"));
    assert!(is_installer("tool.x86_64.rpm"));
    assert!(is_installer("Tool.AppImage"));
    assert_eq!(
        classify_download("tool.deb", 30, STALE_DAYS),
        Some(Risk::Safe)
    );
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
