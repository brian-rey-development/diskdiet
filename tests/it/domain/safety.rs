use diskdiet::domain::safety::is_safe_delete_target;
use std::path::Path;

#[test]
fn delete_guard_protects_home_its_ancestors_and_shared_folders() {
    let home = Some(Path::new("/Users/me"));
    for refused in [
        "/Users/me",
        "/users/ME",
        "/Users",
        "/Users/me/Documents",
        "/Users/me/.cache",
        "/Users/someone",
        "/Volumes/Backup",
        "/Users/me/x/../..",
        "/Users/me/./Documents",
    ] {
        assert!(
            !is_safe_delete_target(Path::new(refused), home),
            "{refused}"
        );
    }
}

#[test]
fn delete_guard_allows_real_cleanup_targets() {
    let home = Some(Path::new("/Users/me"));
    for allowed in [
        "/Users/me/.cache/puppeteer",
        "/Users/me/Develop/app/target",
        "/Applications/Old.app",
        "/Volumes/Work/app/target",
    ] {
        assert!(is_safe_delete_target(Path::new(allowed), home), "{allowed}");
    }
}

#[test]
fn delete_guard_refuses_mount_roots_and_mount_points_at_every_depth() {
    let home = Some(Path::new("/home/me"));
    for refused in [
        "/media",
        "/media/me",
        "/media/me/Drive",
        "/run/media",
        "/run/media/me",
        "/run/media/me/Drive",
        "/RUN/Media/Me/Drive",
        "/mnt",
        "/mnt/x",
        "/Volumes",
        "/Volumes/X",
    ] {
        assert!(
            !is_safe_delete_target(Path::new(refused), home),
            "{refused}"
        );
    }
}

#[test]
fn delete_guard_allows_folders_inside_a_mounted_drive() {
    let home = Some(Path::new("/home/me"));
    for allowed in [
        "/media/me/Drive/app/target",
        "/run/media/me/Drive/app",
        "/mnt/x/app",
        "/Volumes/X/app",
    ] {
        assert!(is_safe_delete_target(Path::new(allowed), home), "{allowed}");
    }
}
