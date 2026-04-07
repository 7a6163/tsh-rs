use crate::{error::*, TshResult};
use portable_pty::{CommandBuilder, PtySize};
use std::io::{Read, Write};
use std::sync::Arc;
use std::sync::Mutex;

/// Cross-platform PTY abstraction
///
/// Reader and writer are extracted once at construction time to avoid
/// repeated `try_clone_reader`/`take_writer` calls in the hot loop.
pub struct Pty {
    reader: Arc<Mutex<Box<dyn Read + Send>>>,
    writer: Arc<Mutex<Box<dyn Write + Send>>>,
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

        // Extract reader and writer once at construction
        let reader = pty_pair
            .master
            .try_clone_reader()
            .map_err(|e| TshError::pty(format!("Failed to clone reader: {e}")))?;

        let writer = pty_pair
            .master
            .take_writer()
            .map_err(|e| TshError::pty(format!("Failed to take writer: {e}")))?;

        Ok(Pty {
            reader: Arc::new(Mutex::new(reader)),
            writer: Arc::new(Mutex::new(writer)),
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
    pub async fn read(&self, buf: &mut [u8]) -> TshResult<usize> {
        let reader = self.reader.clone();
        let buf_len = buf.len();

        let (n, data) = tokio::task::spawn_blocking(move || {
            let mut reader = reader
                .lock()
                .map_err(|e| TshError::pty(format!("Reader lock poisoned: {e}")))?;

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
    pub async fn write(&self, data: &[u8]) -> TshResult<usize> {
        let writer = self.writer.clone();
        let data = data.to_vec();

        let n = tokio::task::spawn_blocking(move || {
            let mut writer = writer
                .lock()
                .map_err(|e| TshError::pty(format!("Writer lock poisoned: {e}")))?;

            match writer.write(&data) {
                Ok(n) => {
                    writer
                        .flush()
                        .map_err(|e| TshError::pty(format!("Flush error: {e}")))?;
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

        let master = self
            .master
            .lock()
            .map_err(|e| TshError::pty(format!("Master lock poisoned: {e}")))?;
        master
            .resize(size)
            .map_err(|e| TshError::pty(format!("Failed to resize PTY: {e}")))
    }
}
