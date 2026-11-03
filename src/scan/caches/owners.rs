use crate::model::{Action, Item};
use crate::process::{DEFAULT_TIMEOUT, run_stdout};
use std::path::Path;

pub(super) const OWNER_RUNNING_NOTE: &str = "Its app is running, quit it first";

const RUNNING_OWNERS: [(&str, &str); 7] = [
    ("Google", "/Google Chrome.app/"),
    ("Firefox", "/Firefox.app/"),
    ("Steam", "/Steam.app/"),
    ("google-chrome", "chrome"),
    ("chromium", "chromium"),
    ("mozilla", "firefox"),
    ("mozilla", "firefox-bin"),
];
const PATH_OWNERS: [(&str, &str); 1] = [("Library/Developer/Xcode/DerivedData", "/Xcode.app/")];
const PS: &str = "ps";
const PATH_MARKER_PREFIX: char = '/';
const PS_ARGS: [&str; 3] = ["-A", "-o", "comm="];

pub(super) fn running_processes() -> String {
    run_stdout(Path::new(PS), &PS_ARGS, DEFAULT_TIMEOUT).unwrap_or_default()
}

fn owned_path(item: &Item) -> Option<&Path> {
    let Action::DeletePath(path) = &item.action else {
        return None;
    };
    let named = RUNNING_OWNERS.iter().any(|(cache, _)| *cache == item.label);
    let located = PATH_OWNERS.iter().any(|(rel, _)| path.ends_with(rel));
    (named || located).then_some(path.as_path())
}

pub(crate) fn owner_is_running_now(item: &Item) -> bool {
    let Some(path) = owned_path(item) else {
        return false;
    };
    let processes = running_processes();
    owner_is_running(&item.label, &processes) || path_owner_is_running(path, &processes)
}

pub fn path_owner_is_running(path: &Path, processes: &str) -> bool {
    PATH_OWNERS
        .iter()
        .any(|(rel, marker)| path.ends_with(rel) && process_matches(processes, marker))
}

pub fn owner_is_running(name: &str, processes: &str) -> bool {
    RUNNING_OWNERS
        .iter()
        .any(|(cache, marker)| *cache == name && process_matches(processes, marker))
}

pub fn process_matches(processes: &str, marker: &str) -> bool {
    if marker.starts_with(PATH_MARKER_PREFIX) {
        return listing_contains_path_fragment(processes, marker);
    }
    listing_has_bare_name(processes, marker)
}

fn listing_contains_path_fragment(processes: &str, fragment: &str) -> bool {
    processes.contains(fragment)
}

fn listing_has_bare_name(processes: &str, name: &str) -> bool {
    let suffix = format!("/{name}");
    processes
        .lines()
        .map(str::trim)
        .any(|line| line == name || line.ends_with(&suffix))
}
