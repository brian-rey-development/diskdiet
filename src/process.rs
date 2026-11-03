use rustix::io::Errno;
use rustix::process::{Pid, WaitId, WaitIdOptions, waitid};
use std::io::{self, Read};
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::{Child, ChildStdout, Command, ExitStatus, Output, Stdio};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::{Arc, Mutex};
use std::time::Duration;

pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(20);

const READ_GRACE: Duration = Duration::from_millis(500);
const READ_CHUNK_BYTES: usize = 64 * 1024;

struct PipeReader {
    buffer: Arc<Mutex<Vec<u8>>>,
    finished: mpsc::Receiver<bool>,
}

pub fn drain_pipe(mut source: impl Read, mut on_chunk: impl FnMut(&[u8])) -> bool {
    let mut chunk = [0u8; READ_CHUNK_BYTES];
    loop {
        match source.read(&mut chunk) {
            Ok(0) => return true,
            Ok(read) => on_chunk(&chunk[..read]),
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(_) => return false,
        }
    }
}

impl PipeReader {
    fn start(stdout: ChildStdout) -> Self {
        let buffer = Arc::new(Mutex::new(Vec::new()));
        let (done, finished) = mpsc::channel();
        let sink = Arc::clone(&buffer);
        std::thread::spawn(move || {
            let complete = drain_pipe(stdout, |bytes| {
                if let Ok(mut collected) = sink.lock() {
                    collected.extend_from_slice(bytes);
                }
            });
            let _ = done.send(complete);
        });
        Self { buffer, finished }
    }

    fn collect(self) -> Option<Vec<u8>> {
        match self.finished.recv_timeout(READ_GRACE) {
            Ok(false) | Err(RecvTimeoutError::Disconnected) => return None,
            Ok(true) | Err(RecvTimeoutError::Timeout) => {}
        }
        let mut bytes = self.buffer.lock().ok()?;
        Some(std::mem::take(&mut *bytes))
    }
}

pub fn announce_exit(mut wait: impl FnMut() -> Result<(), Errno>, exited: mpsc::Sender<()>) {
    loop {
        match wait() {
            Err(Errno::INTR) => {}
            Ok(()) => {
                let _ = exited.send(());
                return;
            }
            Err(_) => return,
        }
    }
}

fn announce_exit_without_reaping(pid: Pid, exited: mpsc::Sender<()>) {
    let options = WaitIdOptions::EXITED | WaitIdOptions::NOWAIT;
    announce_exit(|| waitid(WaitId::Pid(pid), options).map(drop), exited);
}

fn wait_or_kill(child: &mut Child, timeout: Duration) -> Option<ExitStatus> {
    let (exited, exit_announced) = mpsc::channel();
    let pid = Pid::from_child(child);
    std::thread::spawn(move || announce_exit_without_reaping(pid, exited));
    if exit_announced.recv_timeout(timeout).is_ok() {
        return child.wait().ok();
    }
    let _ = child.kill();
    let _ = child.wait();
    None
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
    let reader = PipeReader::start(child.stdout.take()?);
    let status = wait_or_kill(&mut child, timeout)?;
    Some((status, reader.collect()?))
}

pub fn stdout_of(command: &mut Command, timeout: Duration) -> Option<String> {
    let (status, stdout) = run_with_deadline(command, timeout)?;
    status
        .success()
        .then(|| String::from_utf8_lossy(&stdout).into_owned())
}

pub fn run_stdout(program: &Path, args: &[&str], timeout: Duration) -> Option<String> {
    let text = stdout_of(Command::new(program).args(args), timeout)?;
    Some(text.trim().to_owned())
}

pub fn run_shielded(command: &mut Command) -> io::Result<Output> {
    command.process_group(0).output()
}
