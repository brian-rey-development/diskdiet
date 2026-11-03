use rayon::prelude::*;
use std::collections::HashMap;
use std::ffi::{OsStr, OsString};
use std::io::Read;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdout, Command, ExitStatus, Stdio};
use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, Instant};

pub const KNOWN_TOOLS: [&str; 11] = [
    "git",
    "uv",
    "pnpm",
    "npm",
    "brew",
    "docker",
    "colima",
    "ollama",
    "cargo",
    "osascript",
    "mdls",
];

const EXEC_BITS: u32 = 0o111;
const FALLBACK_SHELL: &str = "/bin/sh";
const SHELL_FLAGS: &str = "-lic";
const MISE: &str = "mise";
const MISE_FIXED_LOCATIONS: [&str; 2] = ["/opt/homebrew/bin/mise", "/usr/local/bin/mise"];
const MISE_HOME_LOCATION: &str = ".local/bin/mise";
const LOOKUP_TIMEOUT: Duration = Duration::from_secs(10);
const POLL_INTERVAL: Duration = Duration::from_millis(5);
const READ_GRACE: Duration = Duration::from_millis(500);
const READ_CHUNK_BYTES: usize = 64 * 1024;
const PATH_MARKER: &str = "__DISKDIET_PATH__=";
const PRINT_PATH: &str = "printf '\\n__DISKDIET_PATH__=%s\\n' \"$PATH\"";

#[derive(Debug, Clone, Default)]
pub struct Tools {
    resolved: HashMap<String, PathBuf>,
}

impl Tools {
    pub fn discover() -> Self {
        let login_path = login_shell_path();
        let resolved = KNOWN_TOOLS
            .par_iter()
            .filter_map(|name| {
                let path = resolve(name, login_path.as_deref())?;
                Some(((*name).to_string(), path))
            })
            .collect();
        Self { resolved }
    }

    pub fn from_map(resolved: HashMap<String, PathBuf>) -> Self {
        Self { resolved }
    }

    pub fn get(&self, name: &str) -> Option<&Path> {
        self.resolved.get(name).map(PathBuf::as_path)
    }
}

fn resolve(name: &str, login_path: Option<&OsStr>) -> Option<PathBuf> {
    resolve_on_path(name)
        .or_else(|| login_path.and_then(|path| find_on_path(name, path)))
        .or_else(|| resolve_via_mise(name))
}

fn is_executable(path: &Path) -> bool {
    std::fs::metadata(path)
        .map(|meta| meta.is_file() && meta.permissions().mode() & EXEC_BITS != 0)
        .unwrap_or(false)
}

pub fn find_on_path(name: &str, path_var: &OsStr) -> Option<PathBuf> {
    std::env::split_paths(path_var)
        .filter(|dir| dir.is_absolute())
        .map(|dir| dir.join(name))
        .find(|candidate| is_executable(candidate))
}

fn resolve_on_path(name: &str) -> Option<PathBuf> {
    let path_var = std::env::var_os("PATH")?;
    find_on_path(name, &path_var)
}

pub fn marked_path(output: &str) -> Option<OsString> {
    output
        .lines()
        .find_map(|line| line.trim().strip_prefix(PATH_MARKER))
        .map(OsString::from)
}

fn login_shell_path() -> Option<OsString> {
    let shell = std::env::var_os("SHELL").unwrap_or_else(|| FALLBACK_SHELL.into());
    let mut command = Command::new(shell);
    command.args([SHELL_FLAGS, PRINT_PATH]);
    let (_, stdout) = run_with_deadline(&mut command, LOOKUP_TIMEOUT)?;
    marked_path(&String::from_utf8_lossy(&stdout))
}

fn mise_binary() -> Option<PathBuf> {
    let mut candidates: Vec<PathBuf> = MISE_FIXED_LOCATIONS.iter().map(PathBuf::from).collect();
    if let Some(home) = std::env::var_os("HOME") {
        candidates.push(Path::new(&home).join(MISE_HOME_LOCATION));
    }
    resolve_on_path(MISE).or_else(|| candidates.into_iter().find(|path| is_executable(path)))
}

fn resolve_via_mise(name: &str) -> Option<PathBuf> {
    let mut command = Command::new(mise_binary()?);
    command.args(["which", name]);
    let (status, stdout) = run_with_deadline(&mut command, LOOKUP_TIMEOUT)?;
    let found = last_absolute_path(&String::from_utf8_lossy(&stdout))?;
    let named = found.file_name() == Some(OsStr::new(name));
    (status.success() && named && is_executable(&found)).then_some(found)
}

fn wait_until(child: &mut Child, deadline: Instant) -> Option<ExitStatus> {
    loop {
        if let Some(status) = child.try_wait().ok()? {
            return Some(status);
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return None;
        }
        std::thread::sleep(POLL_INTERVAL);
    }
}

fn spawn_reader(mut stdout: ChildStdout) -> (Arc<Mutex<Vec<u8>>>, mpsc::Receiver<()>) {
    let buffer = Arc::new(Mutex::new(Vec::new()));
    let (done, finished) = mpsc::channel();
    let sink = Arc::clone(&buffer);
    std::thread::spawn(move || {
        let mut chunk = [0u8; READ_CHUNK_BYTES];
        while let Ok(read @ 1..) = stdout.read(&mut chunk) {
            if let Ok(mut bytes) = sink.lock() {
                bytes.extend_from_slice(&chunk[..read]);
            }
        }
        let _ = done.send(());
    });
    (buffer, finished)
}

pub fn run_with_deadline(
    command: &mut Command,
    timeout: Duration,
) -> Option<(ExitStatus, Vec<u8>)> {
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let (buffer, finished) = spawn_reader(child.stdout.take()?);
    let status = wait_until(&mut child, Instant::now() + timeout)?;
    let _ = finished.recv_timeout(READ_GRACE);
    let bytes = buffer.lock().ok()?.clone();
    Some((status, bytes))
}

fn path_in_segment(segment: &str) -> Option<PathBuf> {
    let start = segment.find('/')?;
    Some(PathBuf::from(segment[start..].trim()))
}

pub fn last_absolute_path(output: &str) -> Option<PathBuf> {
    output
        .lines()
        .rev()
        .flat_map(|line| line.rsplit(char::is_control))
        .filter_map(path_in_segment)
        .find(|path| path.is_absolute() && path.exists())
}
