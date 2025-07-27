use crossterm::{
    event::{self, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use indicatif::{ProgressBar, ProgressStyle};
use log::{error, info};
use std::io::{stdout, Write};
use std::path::Path;
use tokio::fs::File;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

use crate::{constants::*, error::*, helpers::NoiseLayerExt, noise::NoiseLayer};

/// PSK Authentication using SHA256
async fn authenticate_with_psk(layer: &mut NoiseLayer, psk: &str) -> TshResult<bool> {
    use sha2::{Digest, Sha256};
    
    // Client: send PSK hash
    let mut hasher = Sha256::new();
    hasher.update(psk.as_bytes());
    hasher.update(b"tsh-client-auth");
    let psk_hash = hasher.finalize();
    
    layer.write_all(&psk_hash).await?;
    
    // Wait for server response
    let mut response = [0u8; 1];
    layer.read_exact(&mut response).await?;
    
    let success = response[0] == 1;
    if success {
        info!("✅ PSK authentication successful");
    } else {
        error!("❌ PSK authentication failed");
    }
    
    Ok(success)
}

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
                
                // Read operation mode from server
                let mut mode_buf = [0u8; 1];
                match layer.read_exact(&mut mode_buf).await {
                    Ok(_) => {
                        let mode = OperationMode::from(mode_buf[0]);
                        info!("🎯 Server requested operation: {mode:?}");
                        
                        // Handle the requested operation
                        let result = match mode {
                            OperationMode::RunShell => {
                                info!("🐚 Starting reverse shell as requested by server");
                                handle_reverse_shell_client(&mut layer).await
                            }
                            _ => {
                                // For other modes, use regular action execution
                                if actions.is_empty() {
                                    error!("No action specified for operation mode: {mode:?}");
                                    Err(TshError::protocol("No action specified"))
                                } else {
                                    execute_action(&mut layer, actions.clone()).await
                                }
                            }
                        };
                        
                        if let Err(e) = result {
                            error!("Operation failed: {e}");
                        }
                    }
                    Err(e) => {
                        error!("Failed to read operation mode from server: {e}");
                    }
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

pub async fn handle_direct_connection(target: &str, port: u16, actions: Vec<&str>, psk: &str) -> TshResult<()> {
    let address = if target.contains(':') {
        target.to_string()
    } else {
        format!("{target}:{port}")
    };
    
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

async fn execute_action(layer: &mut NoiseLayer, actions: Vec<&str>) -> TshResult<()> {
    if actions.is_empty() {
        // Interactive shell mode
        interactive_shell(layer).await
    } else {
        // Parse action
        let action_str = actions.join(" ");
        let parts: Vec<&str> = action_str.split(':').collect();

        match parts.first().copied() {
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
    info!("🐚 Starting interactive shell... Press Ctrl+C to exit");
    
    // Send mode byte in a single message
    layer.write_all(&[OperationMode::RunShell as u8]).await?;
    
    // Enable raw mode for terminal
    enable_raw_mode().map_err(|e| TshError::Io(std::io::Error::new(std::io::ErrorKind::Other, e)))?;
    execute!(stdout(), EnterAlternateScreen)
        .map_err(|e| TshError::Io(std::io::Error::new(std::io::ErrorKind::Other, e)))?;
    
    let result = shell_loop(layer).await;
    
    // Restore terminal
    let _ = disable_raw_mode();
    let _ = execute!(stdout(), LeaveAlternateScreen);
    
    result
}

async fn shell_loop(layer: &mut NoiseLayer) -> TshResult<()> {
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
                        print!("{}", String::from_utf8_lossy(&data));
                        stdout().flush().unwrap();
                    }
                    Err(e) => {
                        error!("Server read error: {e}");
                        break;
                    }
                }
            }
            
            // Read from user input
            user_input = read_user_input() => {
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

async fn read_user_input() -> TshResult<Option<Vec<u8>>> {
    if event::poll(tokio::time::Duration::from_millis(10))
        .map_err(|e| TshError::Io(std::io::Error::new(std::io::ErrorKind::Other, e)))?
    {
        match event::read()
            .map_err(|e| TshError::Io(std::io::Error::new(std::io::ErrorKind::Other, e)))?
        {
            Event::Key(key_event) => match key_event.code {
                KeyCode::Char('c') if key_event.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) => {
                    return Ok(None);
                }
                KeyCode::Char(c) => Ok(Some(vec![c as u8])),
                KeyCode::Enter => Ok(Some(vec![b'\r', b'\n'])),
                KeyCode::Backspace => Ok(Some(vec![8])),
                KeyCode::Tab => Ok(Some(vec![b'\t'])),
                _ => Ok(Some(vec![])),
            },
            _ => Ok(Some(vec![])),
        }
    } else {
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        Ok(Some(vec![]))
    }
}

async fn download_file(layer: &mut NoiseLayer, remote_path: &str, local_dir: &str) -> TshResult<()> {
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
    let local_path = format!("{local_dir}/{remote_filename}");
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
    info!("📁 File downloaded to: {local_path}");
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
    enable_raw_mode().map_err(|e| TshError::Io(std::io::Error::new(std::io::ErrorKind::Other, e)))?;
    execute!(stdout(), EnterAlternateScreen)
        .map_err(|e| TshError::Io(std::io::Error::new(std::io::ErrorKind::Other, e)))?;
    
    let result = shell_loop(layer).await;
    
    // Restore terminal
    let _ = disable_raw_mode();
    let _ = execute!(stdout(), LeaveAlternateScreen);
    
    result
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
    
    stdout().flush().unwrap();
    
    if exit_code != 0 {
        info!("⚠️  Command exited with code: {exit_code}");
    } else {
        info!("✅ Command executed successfully");
    }
    
    Ok(())
}