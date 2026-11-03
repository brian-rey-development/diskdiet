pub mod ignored;
pub mod porcelain;
pub mod status;

use crate::process::stdout_of;
use crate::tools::{self, Tools};
use std::path::Path;
use std::process::Command;
use std::time::Duration;

const GIT_TIMEOUT: Duration = Duration::from_secs(30);
const NO_FSMONITOR: [&str; 2] = ["-c", "core.fsmonitor=false"];

pub(crate) fn program(tools: &Tools) -> &Path {
    tools.get_or(tools::GIT, tools::GIT_FALLBACK)
}

fn command(git: &Path, dir: &Path, args: &[&str]) -> Command {
    let mut command = Command::new(git);
    command
        .arg("-C")
        .arg(dir)
        .args(NO_FSMONITOR)
        .args(args)
        .env("GIT_OPTIONAL_LOCKS", "0")
        .env("GIT_TERMINAL_PROMPT", "0");
    command
}

pub(crate) fn run(git: &Path, dir: &Path, args: &[&str]) -> Option<String> {
    stdout_of(&mut command(git, dir, args), GIT_TIMEOUT)
}
