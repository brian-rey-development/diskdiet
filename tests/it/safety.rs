use diskdiet::safety::is_safe_delete_target;
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
