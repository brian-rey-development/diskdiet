use anyhow::Result;
use diskdiet::actions::{applescript_escape, prune_names, trash_script};
use diskdiet::freedesktop_trash::{deletion_date_now, empty, encode_path};
use std::fs;
use std::path::Path;
use tempfile::tempdir;

const DELETION_DATE_LEN: usize = "2026-01-02T03:04:05".len();
const DATE_TIME_SEPARATOR_INDEX: usize = 10;

#[test]
fn deletion_date_uses_a_t_separator_and_no_spaces() {
    let date = deletion_date_now();
    assert_eq!(date.len(), DELETION_DATE_LEN);
    assert_eq!(date.as_bytes()[DATE_TIME_SEPARATOR_INDEX], b'T');
    assert!(!date.contains(' '));
}

#[test]
fn encoded_paths_percent_encode_spaces_and_non_ascii_bytes() {
    assert_eq!(encode_path(Path::new("/a b/é")), "/a%20b/%C3%A9");
    assert_eq!(encode_path(Path::new("/keep-_.~/ok")), "/keep-_.~/ok");
}

#[test]
fn emptying_the_trash_removes_the_directory_size_cache() -> Result<()> {
    let trash = tempdir()?;
    fs::write(trash.path().join("directorysizes"), "1 2 name\n")?;
    empty(trash.path())?;
    assert!(!trash.path().join("directorysizes").exists());
    Ok(())
}

#[test]
fn applescript_text_escapes_quotes_and_backslashes() {
    assert_eq!(applescript_escape(r#"a"b\c"#), r#"a\"b\\c"#);
    assert!(trash_script(Path::new("/tmp/x\"y")).contains(r#"/tmp/x\"y"#));
}

#[test]
fn prune_preview_lines_without_the_prefix_are_ignored() {
    let output = "Removing worktrees/a: gitdir file points to non-existent location\nnoise\n";
    assert_eq!(prune_names(output), vec!["a".to_owned()]);
}
