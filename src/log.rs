use crate::time::{format_timestamp, now_secs};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

pub fn log_path(home: &Path) -> PathBuf {
    crate::platform::log_path(home)
}

fn append(path: &Path, line: &str) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    let single_line = line.replace(['\n', '\r'], " ");
    writeln!(file, "{}  {single_line}", format_timestamp(now_secs()))
}

pub fn record(home: &Path, line: &str) {
    let _ = append(&log_path(home), line);
}
