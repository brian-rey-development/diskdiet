use anyhow::{Result, anyhow};
use diskdiet::process::{announce_exit, drain_pipe, run_with_deadline};
use rustix::io::Errno;
use std::io::{self, Read};
use std::process::Command;
use std::sync::mpsc;
use std::time::Duration;

const SHORT_DEADLINE: Duration = Duration::from_secs(5);

struct Script {
    steps: Vec<io::Result<Vec<u8>>>,
}

impl Read for Script {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        if self.steps.is_empty() {
            return Ok(0);
        }
        let bytes = self.steps.remove(0)?;
        buffer[..bytes.len()].copy_from_slice(&bytes);
        Ok(bytes.len())
    }
}

fn collect(steps: Vec<io::Result<Vec<u8>>>) -> (Vec<u8>, bool) {
    let mut collected = Vec::new();
    let complete = drain_pipe(Script { steps }, |bytes| collected.extend_from_slice(bytes));
    (collected, complete)
}

#[test]
fn an_interrupted_read_is_retried_and_no_output_is_lost() {
    let interrupted = Err(io::Error::from(io::ErrorKind::Interrupted));
    let steps = vec![Ok(b"ab".to_vec()), interrupted, Ok(b"cd".to_vec())];
    assert_eq!(collect(steps), (b"abcd".to_vec(), true));
}

#[test]
fn a_failed_read_is_reported_as_incomplete() {
    let broken = Err(io::Error::from(io::ErrorKind::BrokenPipe));
    let (_, complete) = collect(vec![Ok(b"ab".to_vec()), broken]);
    assert!(!complete);
}

#[test]
fn the_exit_watcher_announces_only_after_a_successful_wait() {
    let (sender, receiver) = mpsc::channel();
    let mut attempts = vec![Err(Errno::INTR), Ok(())].into_iter();
    announce_exit(|| attempts.next().unwrap_or(Err(Errno::CHILD)), &sender);
    assert!(receiver.try_recv().is_ok());
}

#[test]
fn the_exit_watcher_stays_silent_when_waiting_fails() {
    let (sender, receiver) = mpsc::channel();
    announce_exit(|| Err(Errno::CHILD), &sender);
    drop(sender);
    assert_eq!(receiver.try_recv(), Err(mpsc::TryRecvError::Disconnected));
}

#[test]
fn run_with_deadline_returns_the_output_of_a_finished_command() -> Result<()> {
    let mut echo = Command::new("/bin/echo");
    echo.arg("hello");
    let (status, stdout) =
        run_with_deadline(&mut echo, SHORT_DEADLINE).ok_or_else(|| anyhow!("echo timed out"))?;
    assert!(status.success());
    assert_eq!(stdout, b"hello\n");
    Ok(())
}
