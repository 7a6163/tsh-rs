use crate::{error::*, TshResult};
use portable_pty::{CommandBuilder, PtySize};
use std::io::{Read, Write};
use std::sync::Arc;
use tokio::sync::Mutex;

/// Cross-platform PTY abstraction
pub struct Pty {
    master: Arc<Mutex<Box<dyn portable_pty::MasterPty + Send>>>,
}

impl Pty {
    /// Create a new PTY with shell
    pub fn new() -> TshResult<Self> {
        let pty_system = portable_pty::native_pty_system();

        let pty_size = PtySize {
            rows: 24,
            cols: 80,
            pixel_width: 0,
            pixel_height: 0,
        };

        let pty_pair = pty_system
            .openpty(pty_size)
            .map_err(|e| TshError::pty(format!("Failed to create PTY: {e}")))?;

        // Spawn shell
        let mut cmd = CommandBuilder::new(Self::get_shell_command());
        cmd.env("TERM", "xterm");

        let _child = pty_pair
            .slave
            .spawn_command(cmd)
            .map_err(|e| TshError::pty(format!("Failed to spawn shell: {e}")))?;

        Ok(Pty {
            master: Arc::new(Mutex::new(pty_pair.master)),
        })
    }

    /// Get the appropriate shell command for the platform
    fn get_shell_command() -> &'static str {
        if cfg!(windows) {
            "cmd.exe"
        } else {
            "/bin/sh"
        }
    }

    /// Read data from PTY
    pub async fn read(&mut self, buf: &mut [u8]) -> TshResult<usize> {
        let master = self.master.clone();
        let buf_len = buf.len();

        let (n, data) = tokio::task::spawn_blocking(move || {
            let master = master.blocking_lock();
            let mut reader = master
                .try_clone_reader()
                .map_err(|e| TshError::pty(format!("Failed to clone reader: {e}")))?;

            let mut temp_buf = vec![0u8; buf_len];
            match reader.read(&mut temp_buf) {
                Ok(n) => Ok((n, temp_buf)),
                Err(e) => Err(TshError::pty(format!("Read error: {e}"))),
            }
        })
        .await
        .map_err(|e| TshError::pty(format!("Task join error: {e}")))??;

        buf[..n].copy_from_slice(&data[..n]);
        Ok(n)
    }

    /// Write data to PTY
    pub async fn write(&mut self, data: &[u8]) -> TshResult<usize> {
        let master = self.master.clone();
        let data = data.to_vec();

        let n = tokio::task::spawn_blocking(move || {
            let master = master.blocking_lock();
            let mut writer = master
                .take_writer()
                .map_err(|e| TshError::pty(format!("Failed to get writer: {e}")))?;

            match writer.write(&data) {
                Ok(n) => {
                    let _ = writer.flush();
                    Ok(n)
                }
                Err(e) => Err(TshError::pty(format!("Write error: {e}"))),
            }
        })
        .await
        .map_err(|e| TshError::pty(format!("Task join error: {e}")))??;

        Ok(n)
    }

    /// Resize the PTY
    pub async fn resize(&self, rows: u16, cols: u16) -> TshResult<()> {
        let size = PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        };

        let master = self.master.lock().await;
        master
            .resize(size)
            .map_err(|e| TshError::pty(format!("Failed to resize PTY: {e}")))
    }
}
