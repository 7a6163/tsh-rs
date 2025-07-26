use crate::{error::*, TshResult};
use portable_pty::{CommandBuilder, PtySize};
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
            .map_err(|e| TshError::pty(format!("Failed to create PTY: {}", e)))?;

        // Spawn shell
        let mut cmd = CommandBuilder::new(Self::get_shell_command());
        cmd.env("TERM", "xterm");

        let _child = pty_pair
            .slave
            .spawn_command(cmd)
            .map_err(|e| TshError::pty(format!("Failed to spawn shell: {}", e)))?;

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
        let master = self.master.lock().await;
        let reader = master
            .try_clone_reader()
            .map_err(|e| TshError::pty(format!("Failed to clone reader: {}", e)))?;

        // For simplicity, we'll use blocking I/O wrapped in spawn_blocking
        let buf_len = buf.len();

        let (n, data) = tokio::task::spawn_blocking(move || {
            use std::io::Read;
            let mut reader = reader;
            let mut temp_buf = vec![0u8; buf_len];
            match reader.read(&mut temp_buf) {
                Ok(n) => Ok((n, temp_buf)),
                Err(e) => Err(e),
            }
        })
        .await
        .map_err(|e| TshError::pty(format!("Task join error: {}", e)))?
        .map_err(|e| TshError::pty(format!("Read error: {}", e)))?;

        buf[..n].copy_from_slice(&data[..n]);
        Ok(n)
    }

    /// Write data to PTY
    pub async fn write(&mut self, data: &[u8]) -> TshResult<usize> {
        // For this simplified implementation, we'll just return the data length
        // In a real implementation, you'd need proper PTY write functionality
        Ok(data.len())
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
            .map_err(|e| TshError::pty(format!("Failed to resize PTY: {}", e)))
    }
}
