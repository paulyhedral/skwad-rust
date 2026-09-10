use std::ffi::OsString;
use std::io::Read;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use crate::consts::DEFAULT_TIMEOUT;
use crate::error::{GitError, Result};

const POLL_INTERVAL: Duration = Duration::from_millis(10);

/// Runs `git` in a fixed working directory with a wall-clock timeout.
///
/// Output streams drain on worker threads so a full pipe buffer never wedges
/// the child; the main thread polls for exit and kills on timeout.
#[derive(Debug, Clone)]
pub struct Runner {
    cwd: PathBuf,
    timeout: Duration,
    program: OsString,
}

impl Runner {
    pub fn new(cwd: impl Into<PathBuf>) -> Self {
        Self {
            cwd: cwd.into(),
            timeout: DEFAULT_TIMEOUT,
            program: OsString::from("git"),
        }
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    #[cfg(test)]
    pub(crate) fn with_program(mut self, program: impl Into<OsString>) -> Self {
        self.program = program.into();
        self
    }

    pub fn cwd(&self) -> &PathBuf {
        &self.cwd
    }

    /// Runs `git <args>`. Returns stdout trimmed of surrounding whitespace.
    ///
    /// Non-zero exit -> [`GitError::Command`] carrying stderr (or stdout when
    /// stderr is empty) and the exit code. Exceeding the timeout kills the
    /// process and yields [`GitError::Timeout`].
    pub fn run(&self, args: &[&str]) -> Result<String> {
        let label = display_command(args);

        let mut child = Command::new(&self.program)
            .args(args)
            .current_dir(&self.cwd)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        let mut stdout_pipe = child.stdout.take().expect("stdout piped");
        let mut stderr_pipe = child.stderr.take().expect("stderr piped");
        let stdout_reader = thread::spawn(move || read_to_string(&mut stdout_pipe));
        let stderr_reader = thread::spawn(move || read_to_string(&mut stderr_pipe));

        let deadline = Instant::now() + self.timeout;
        let status = loop {
            if let Some(status) = child.try_wait()? {
                break status;
            }

            if Instant::now() >= deadline {
                let _ = child.kill();
                let _ = child.wait();
                drop(stdout_reader);
                drop(stderr_reader);
                return Err(GitError::Timeout { command: label });
            }

            thread::sleep(POLL_INTERVAL);
        };

        let stdout = stdout_reader.join().unwrap_or_default();
        let stderr = stderr_reader.join().unwrap_or_default();

        if status.success() {
            return Ok(stdout.trim().to_owned());
        }

        let output = if stderr.trim().is_empty() {
            stdout
        } else {
            stderr
        };

        Err(GitError::Command {
            command: label,
            output: output.trim().to_owned(),
            code: status.code().unwrap_or(-1),
        })
    }
}

fn read_to_string(pipe: &mut impl Read) -> String {
    let mut buf = String::new();
    let _ = pipe.read_to_string(&mut buf);
    buf
}

fn display_command(args: &[&str]) -> String {
    args.join(" ")
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::Runner;
    use crate::error::GitError;

    #[test]
    fn timeout_kills_process_and_names_command() {
        let dir = tempfile::tempdir().unwrap();
        let runner = Runner::new(dir.path())
            .with_program("sleep")
            .with_timeout(Duration::from_millis(50));

        let started = std::time::Instant::now();
        let err = runner.run(&["5"]).unwrap_err();

        assert!(
            started.elapsed() < Duration::from_secs(2),
            "did not abort early"
        );
        match err {
            GitError::Timeout { command } => assert_eq!(command, "5"),
            other => panic!("expected Timeout, got {other:?}"),
        }
    }
}
