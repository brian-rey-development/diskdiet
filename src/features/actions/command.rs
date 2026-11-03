use crate::domain::model::Outcome;
use crate::infra::sys::process::run_shielded;
use std::path::Path;
use std::process::{Command, Output};

const STDERR_TAIL_CHARS: usize = 300;
const PERMISSION_MARKERS: [&str; 2] = ["Operation not permitted", "(-54)"];
const AUTOMATION_MARKER: &str = "(-1743)";
const AUTOMATION_BLOCKED: &str = "macOS blocked controlling Finder. Allow your terminal in System Settings > Privacy & Security > Automation";

pub(super) fn run_command(program: &Path, args: &[String]) -> Outcome {
    capture_shielded_from_ctrl_c(Command::new(program).args(args))
}

pub(super) fn capture_shielded_from_ctrl_c(command: &mut Command) -> Outcome {
    match run_shielded(command) {
        Ok(output) => output_outcome(&output),
        Err(error) => Outcome::Failed(error.to_string()),
    }
}

fn output_outcome(output: &Output) -> Outcome {
    if output.status.success() {
        return Outcome::Done;
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    if stderr.contains(AUTOMATION_MARKER) {
        return Outcome::Failed(AUTOMATION_BLOCKED.to_owned());
    }
    if needs_permission(&stderr) {
        return Outcome::NeedsFullDiskAccess;
    }
    let detail = last_chars(&stderr);
    if detail.is_empty() {
        return Outcome::Failed(format!("exited with {}", output.status));
    }
    Outcome::Failed(detail)
}

fn needs_permission(stderr: &str) -> bool {
    PERMISSION_MARKERS
        .iter()
        .any(|marker| stderr.contains(marker))
}

fn last_chars(text: &str) -> String {
    let trimmed = text.trim();
    let count = trimmed.chars().count();
    if count <= STDERR_TAIL_CHARS {
        return trimmed.to_owned();
    }
    trimmed.chars().skip(count - STDERR_TAIL_CHARS).collect()
}
