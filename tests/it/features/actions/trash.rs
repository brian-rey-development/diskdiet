use diskdiet::features::actions::{applescript_escape, trash_script};
use std::path::Path;

#[test]
fn applescript_escape_handles_quotes_and_backslashes() {
    assert_eq!(applescript_escape(r#"a"b"#), r#"a\"b"#);
    assert_eq!(applescript_escape(r"a\b"), r"a\\b");
    assert_eq!(applescript_escape(r#"a\"b"#), r#"a\\\"b"#);
    assert_eq!(applescript_escape("/plain/path"), "/plain/path");
}

#[test]
fn trash_script_wraps_the_escaped_path() {
    let script = trash_script(Path::new(r#"/Applications/My "App"\x.app"#));
    assert_eq!(
        script,
        r#"tell application "Finder" to delete POSIX file "/Applications/My \"App\"\\x.app""#
    );
}

#[test]
fn applescript_text_escapes_quotes_and_backslashes() {
    assert_eq!(applescript_escape(r#"a"b\c"#), r#"a\"b\\c"#);
    assert!(trash_script(Path::new("/tmp/x\"y")).contains(r#"/tmp/x\"y"#));
}
