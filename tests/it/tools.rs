use crate::support::RWX_MODE;
use crate::support::fixtures::set_mode;
use anyhow::Result;
use diskdiet::tools::{Tools, find_on_path, last_absolute_path, path_from_login_shell_output};
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::tempdir;

#[test]
fn login_shell_output_yields_the_marked_path_despite_rc_noise() {
    let noisy = "Welcome back!\n\n__DISKDIET_PATH__=/opt/homebrew/bin:/usr/bin\nbye\n";
    assert_eq!(
        path_from_login_shell_output(noisy),
        Some(OsString::from("/opt/homebrew/bin:/usr/bin"))
    );
    assert_eq!(path_from_login_shell_output("no marker here"), None);
}

#[test]
fn find_on_path_requires_an_executable_regular_file() -> Result<()> {
    let dir = tempdir()?;
    let tool = dir.path().join("mytool");
    fs::write(&tool, "#!/bin/sh\n")?;
    let path_var = std::env::join_paths([dir.path()])?;
    assert_eq!(find_on_path("mytool", &path_var), None);
    set_mode(&tool, RWX_MODE)?;
    assert_eq!(find_on_path("mytool", &path_var), Some(tool));
    Ok(())
}

#[test]
fn last_absolute_path_skips_terminal_noise() -> Result<()> {
    let dir = tempdir()?;
    let real = dir.path().join("pnpm");
    fs::write(&real, "")?;
    let noisy = format!(
        "\u{1b}]1337;ShellIntegration\u{7}welcome banner\n\u{1b}[0m{}\u{1b}[0m\r\nshell: trailing text\n",
        real.display()
    );
    assert_eq!(last_absolute_path(&noisy), Some(real));
    assert_eq!(last_absolute_path("alias pnpm=foo\n/no/such/file\n"), None);
    Ok(())
}

#[test]
fn tools_from_map_and_get_roundtrip() {
    let map = [("git".to_string(), PathBuf::from("/usr/bin/git"))].into();
    let tools = Tools::from_map(map);
    assert_eq!(tools.get("git"), Some(Path::new("/usr/bin/git")));
    assert_eq!(tools.get("nope"), None);
}

#[test]
fn tools_discover_finds_git_and_never_invents_tools() {
    let tools = Tools::discover();
    assert!(tools.get("git").is_some_and(Path::exists));
    assert_eq!(tools.get("definitely-not-a-tool"), None);
}
