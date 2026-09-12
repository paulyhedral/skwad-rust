use std::io::{Read, Write};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::thread;

use portable_pty::{Child, CommandBuilder, PtySize, native_pty_system};

use crate::{Result, TerminalError, TerminalTransport};

pub struct PtyTransport {
    writer: Mutex<Box<dyn Write + Send>>,
    child: Arc<Mutex<Box<dyn Child + Send + Sync>>>,
}

impl PtyTransport {
    pub fn spawn<Output, Exit>(
        folder: impl AsRef<Path>,
        shell: impl Into<String>,
        on_output: Output,
        on_exit: Exit,
    ) -> Result<Self>
    where
        Output: Fn(&[u8]) + Send + Sync + 'static,
        Exit: Fn(Option<i32>) + Send + Sync + 'static,
    {
        let pty_system = native_pty_system();
        let pair = pty_system
            .openpty(PtySize {
                rows: 24,
                cols: 80,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|error| TerminalError::Transport(error.to_string()))?;
        let mut command = CommandBuilder::new(shell.into());
        command.arg("-i");
        command.cwd(folder.as_ref());
        let child = pair
            .slave
            .spawn_command(command)
            .map_err(|error| TerminalError::Transport(error.to_string()))?;
        let child = Arc::new(Mutex::new(child));
        let mut reader = pair
            .master
            .try_clone_reader()
            .map_err(|error| TerminalError::Transport(error.to_string()))?;
        let child_for_wait = Arc::clone(&child);
        thread::spawn(move || {
            let mut buffer = [0_u8; 4096];
            loop {
                match reader.read(&mut buffer) {
                    Ok(0) | Err(_) => break,
                    Ok(size) => on_output(&buffer[..size]),
                }
            }
            let code = child_for_wait
                .lock()
                .ok()
                .and_then(|mut child| child.wait().ok())
                .map(|status| status.exit_code() as i32);
            on_exit(code);
        });
        let writer = pair
            .master
            .take_writer()
            .map_err(|error| TerminalError::Transport(error.to_string()))?;
        Ok(Self {
            writer: Mutex::new(writer),
            child,
        })
    }
}

impl Drop for PtyTransport {
    fn drop(&mut self) {
        if let Ok(mut child) = self.child.lock() {
            let _ = child.kill();
        }
    }
}

impl TerminalTransport for PtyTransport {
    fn send_text(&mut self, text: &str) -> Result<()> {
        self.writer
            .lock()
            .map_err(|_| TerminalError::Transport("PTY writer lock poisoned".to_string()))?
            .write_all(text.as_bytes())
            .map_err(|error| TerminalError::Transport(error.to_string()))
    }

    fn send_return(&mut self) -> Result<()> {
        self.send_text("\r")
    }

    fn terminate(&mut self) -> Result<()> {
        self.child
            .lock()
            .map_err(|_| TerminalError::Transport("PTY child lock poisoned".to_string()))?
            .kill()
            .map_err(|error| TerminalError::Transport(error.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc;
    use std::time::Duration;

    use super::*;

    #[test]
    fn pty_forwards_output_and_exit_status() {
        let folder = tempfile::tempdir().unwrap();
        let (output_tx, output_rx) = mpsc::channel();
        let (exit_tx, exit_rx) = mpsc::channel();
        let mut transport = PtyTransport::spawn(
            folder.path(),
            "/bin/sh",
            move |bytes| {
                let _ = output_tx.send(bytes.to_vec());
            },
            move |status| {
                let _ = exit_tx.send(status);
            },
        )
        .unwrap();

        transport.send_text("printf ready; exit 3\n").unwrap();
        let status = exit_rx.recv_timeout(Duration::from_secs(5)).unwrap();
        let output = output_rx.try_iter().flatten().collect::<Vec<_>>();

        assert_eq!(status, Some(3));
        assert!(String::from_utf8_lossy(&output).contains("ready"));
    }
}
