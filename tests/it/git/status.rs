use diskdiet::git::status::count_status;

#[test]
fn status_counts_split_dirty_from_risky_ignored() {
    let text = " M src/a.rs\0?? new.txt\0!! target/\0!! .env\0!! web/node_modules/\0R  renamed.rs\0old name.rs\0";
    let counts = count_status(text);
    assert_eq!((counts.dirty, counts.risky_ignored), (3, 1));
    assert_eq!(count_status(""), Default::default());
}

#[test]
fn malformed_status_fields_count_as_dirty() {
    assert_eq!(count_status("X\0").dirty, 1);
}
