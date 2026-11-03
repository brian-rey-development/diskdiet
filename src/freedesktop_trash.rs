use crate::fsutil::remove_any;
use crate::time;
use std::ffi::{OsStr, OsString};
use std::fmt::Write as _;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};

const FILES_DIR: &str = "files";
const INFO_DIR: &str = "info";
const INFO_SUFFIX: &str = ".trashinfo";
const EXPUNGED_DIR: &str = "expunged";
const EMPTIED_DIRS: [&str; 3] = [FILES_DIR, INFO_DIR, EXPUNGED_DIR];
const UNENCODED_PUNCTUATION: &[u8] = b"-_.~/";
const SIZES_CACHE: &str = "directorysizes";
const MAX_NAME_ATTEMPTS: u32 = 10_000;

pub fn encode_path(path: &Path) -> String {
    path.as_os_str()
        .as_bytes()
        .iter()
        .fold(String::new(), |mut out, &byte| {
            if byte.is_ascii_alphanumeric() || UNENCODED_PUNCTUATION.contains(&byte) {
                out.push(char::from(byte));
            } else {
                let _ = write!(out, "%{byte:02X}");
            }
            out
        })
}

pub fn deletion_date_now() -> String {
    time::format_timestamp(time::now_secs()).replace(' ', "T")
}

pub fn info_text(path: &Path, deleted_at: &str) -> String {
    format!(
        "[Trash Info]\nPath={}\nDeletionDate={deleted_at}\n",
        encode_path(path)
    )
}

fn candidate_name(base: &OsStr, attempt: u32) -> OsString {
    let mut name = base.to_os_string();
    if attempt > 0 {
        name.push(format!(".{}", attempt + 1));
    }
    name
}

fn info_path(trash: &Path, name: &OsStr) -> PathBuf {
    let mut file = name.to_os_string();
    file.push(INFO_SUFFIX);
    trash.join(INFO_DIR).join(file)
}

fn try_claim(trash: &Path, name: &OsStr, text: &str) -> io::Result<bool> {
    let info = info_path(trash, name);
    let mut file = match OpenOptions::new().write(true).create_new(true).open(&info) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => return Ok(false),
        Err(error) => return Err(error),
    };
    if trash.join(FILES_DIR).join(name).exists() {
        drop(file);
        fs::remove_file(&info)?;
        return Ok(false);
    }
    file.write_all(text.as_bytes())?;
    Ok(true)
}

fn claim_name(trash: &Path, base: &OsStr, text: &str) -> io::Result<OsString> {
    for attempt in 0..MAX_NAME_ATTEMPTS {
        let name = candidate_name(base, attempt);
        if try_claim(trash, &name, text)? {
            return Ok(name);
        }
    }
    Err(io::Error::from(io::ErrorKind::AlreadyExists))
}

pub fn move_to(trash: &Path, path: &Path, deleted_at: &str) -> io::Result<()> {
    fs::create_dir_all(trash.join(FILES_DIR))?;
    fs::create_dir_all(trash.join(INFO_DIR))?;
    let base = path
        .file_name()
        .ok_or_else(|| io::Error::from(io::ErrorKind::InvalidInput))?;
    let name = claim_name(trash, base, &info_text(path, deleted_at))?;
    fs::rename(path, trash.join(FILES_DIR).join(&name)).inspect_err(|_| {
        let _ = fs::remove_file(info_path(trash, &name));
    })
}

fn ignore_not_found<T>(result: io::Result<T>) -> io::Result<Option<T>> {
    match result {
        Ok(value) => Ok(Some(value)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

fn clear_dir(dir: &Path) -> io::Result<()> {
    let Some(entries) = ignore_not_found(fs::read_dir(dir))? else {
        return Ok(());
    };
    for entry in entries {
        remove_any(&entry?.path())?;
    }
    Ok(())
}

pub fn empty(trash: &Path) -> io::Result<()> {
    for dir in EMPTIED_DIRS {
        clear_dir(&trash.join(dir))?;
    }
    ignore_not_found(fs::remove_file(trash.join(SIZES_CACHE))).map(drop)
}
