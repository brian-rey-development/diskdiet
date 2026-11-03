use anyhow::Result;
use diskdiet::log;
use std::fs;
use tempfile::tempdir;

#[test]
fn log_record_appends_timestamped_lines_and_creates_folders() -> Result<()> {
    let home = tempdir()?;
    log::record(home.path(), "delete /tmp/a -> Done");
    log::record(home.path(), "multi\nline");
    let text = fs::read_to_string(log::log_path(home.path()))?;
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines.len(), 2);
    assert!(lines[0].ends_with("  delete /tmp/a -> Done"));
    assert!(lines[1].ends_with("  multi line"));
    assert_eq!(lines[0].as_bytes()[4], b'-');
    assert_eq!(lines[0].as_bytes()[13], b':');
    Ok(())
}
