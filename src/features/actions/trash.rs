use super::command::capture_shielded_from_ctrl_c;
use super::io_outcome;
use crate::domain::model::Outcome;
use crate::infra::fs::freedesktop_trash;
use crate::infra::sys::platform::{self, IS_MACOS, NO_HOME};
use crate::infra::sys::tools::{self, Tools};
use std::io;
use std::path::Path;
use std::process::Command;

const EMPTY_TRASH_SCRIPT: &str = "tell application \"Finder\" to empty trash";
const OTHER_DRIVE: &str =
    "it is on another drive, so it cannot go to your Trash. Delete it by hand";

pub fn applescript_escape(text: &str) -> String {
    text.replace('\\', "\\\\").replace('"', "\\\"")
}

pub fn trash_script(path: &Path) -> String {
    let escaped = applescript_escape(&path.to_string_lossy());
    format!("tell application \"Finder\" to delete POSIX file \"{escaped}\"")
}

pub(super) fn move_to_trash(tools: &Tools, path: &Path) -> Outcome {
    if IS_MACOS {
        return run_osascript(tools, &trash_script(path));
    }
    with_home(|home| {
        let deleted_at = freedesktop_trash::deletion_date_now();
        let trash = platform::trash_dir(home);
        trash_outcome(freedesktop_trash::move_to(&trash, path, &deleted_at))
    })
}

pub(super) fn empty_trash(tools: &Tools) -> Outcome {
    if IS_MACOS {
        return run_osascript(tools, EMPTY_TRASH_SCRIPT);
    }
    with_home(|home| trash_outcome(freedesktop_trash::empty(&platform::trash_dir(home))))
}

fn with_home(run: impl FnOnce(&Path) -> Outcome) -> Outcome {
    match platform::home_dir() {
        Some(home) => run(&home),
        None => Outcome::Failed(NO_HOME.to_owned()),
    }
}

fn run_osascript(tools: &Tools, script: &str) -> Outcome {
    let program = tools.get_or(tools::OSASCRIPT, tools::OSASCRIPT_FALLBACK);
    capture_shielded_from_ctrl_c(Command::new(program).arg("-e").arg(script))
}

fn trash_outcome(result: io::Result<()>) -> Outcome {
    match result {
        Ok(()) => Outcome::Done,
        Err(error) if error.kind() == io::ErrorKind::CrossesDevices => {
            Outcome::Failed(OTHER_DRIVE.to_owned())
        }
        Err(error) => io_outcome(&error),
    }
}
