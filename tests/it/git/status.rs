use diskdiet::git::status::count_status;
use std::path::Path;

const NO_MANIFESTS: &str = "/nonexistent-diskdiet-worktree";

#[test]
fn status_counts_split_dirty_from_risky_ignored() {
    let text = " M src/a.rs\0?? new.txt\0!! .env\0!! web/node_modules/\0!! .next/\0R  renamed.rs\0old name.rs\0";
    let counts = count_status(text, Path::new(NO_MANIFESTS));
    assert_eq!((counts.dirty, counts.risky_ignored), (3, 1));
    assert_eq!(
        count_status("", Path::new(NO_MANIFESTS)),
        Default::default()
    );
}

#[test]
fn an_ambiguous_ignored_folder_without_a_manifest_is_risky() {
    let counts = count_status("!! target/\0", Path::new(NO_MANIFESTS));
    assert_eq!(counts.risky_ignored, 1);
}

#[test]
fn malformed_status_fields_count_as_dirty() {
    assert_eq!(count_status("X\0", Path::new(NO_MANIFESTS)).dirty, 1);
}
