use crate::fmt::tilde_path;
use crate::size::path_usage;
use std::path::Path;

pub(crate) const DETAIL_SEPARATOR: &str = " · ";
pub(crate) const GIT_DIR: &str = ".git";

pub(crate) fn measured(path: &Path) -> Option<u64> {
    if !path.exists() {
        return None;
    }
    let bytes = path_usage(path);
    (bytes > 0).then_some(bytes)
}

pub(crate) fn located_detail(note: &str, path: &Path, home: &Path) -> String {
    format!("{note}{DETAIL_SEPARATOR}{}", tilde_path(path, home))
}

pub(crate) fn file_label(path: &Path) -> Option<String> {
    Some(path.file_name()?.to_string_lossy().into_owned())
}
