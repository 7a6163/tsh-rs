use clap::{Arg, Command};
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

// Import our library modules
use tsh_rs::{constants::*, error::*, pel::PktEncLayer};

#[tokio::main]
async fn main() -> TshResult<()> {
    env_logger::init();

    let matches = Command::new("tsh")
        .version("0.1.0")
        .author("Your Name <your.email@example.com>")
        .about("Tiny Shell - Remote shell access client")
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
            Arg::new("target")
                .value_name("TARGET")
                .help("Target hostname or 'cb' for connect-back")
                .required(true)
                .index(1),
        )
        .arg(
            Arg::new("action")
                .value_name("ACTION")
                .help("Action: get <source> <dest> | put <source> <dest> | [command]")
                .num_args(0..)
                .index(2),
        )
        .get_matches();

    let secret = matches.get_one::<String>("secret").unwrap().clone();
    let port: u16 = matches
        .get_one::<String>("port")
        .unwrap()
        .parse()
        .map_err(|_| TshError::system("Invalid port number"))?;

    let target = matches.get_one::<String>("target").unwrap();
    let actions: Vec<&str> = matches
        .get_many::<String>("action")
        .unwrap_or_default()
        .map(|s| s.as_str())
        .collect();

    if target == "cb" {
        handle_connect_back(secret, port, actions).await
    } else {
        handle_direct_connection(target, secret, port, actions).await
    }
}

async fn handle_connect_back(secret: String, port: u16, actions: Vec<&str>) -> TshResult<()> {
    info!("Starting connect-back mode on port {}", port);

    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", port))
        .await
        .map_err(|_| TshError::network("Address already in use"))?;

    println!("Waiting for the server to connect...");

    let (stream, _) = listener
        .accept()
        .await
        .map_err(|e| TshError::network(format!("Failed to accept connection: {}", e)))?;

    let mut layer = PktEncLayer::new(stream, secret);
    layer.handshake(false).await?;

    // Authentication check (simplified)
    print!("Password: ");
    std::io::stdout().flush().unwrap();

    println!("connected.");

    execute_action(&mut layer, actions).await
}

async fn handle_direct_connection(
    target: &str,
    secret: String,
    port: u16,
    actions: Vec<&str>,
) -> TshResult<()> {
    let address = if target.contains(':') {
        target.to_string()
    } else {
        format!("{}:{}", target, port)
    };

    info!("Connecting to {}", address);

    let mut layer = PktEncLayer::connect(&address, secret, false).await?;

    // Authentication check (simplified)
    print!("Password:");
    std::io::stdout().flush().unwrap();

    execute_action(&mut layer, actions).await
}

async fn execute_action(layer: &mut PktEncLayer, actions: Vec<&str>) -> TshResult<()> {
    if actions.is_empty() {
        // Interactive shell mode
        run_interactive_shell(layer).await
    } else {
        match actions[0] {
            "get" => {
                if actions.len() != 3 {
                    return Err(TshError::system("Usage: get <source-file> <dest-dir>"));
                }
                download_file(layer, actions[1], actions[2]).await
            }
            "put" => {
                if actions.len() != 3 {
                    return Err(TshError::system("Usage: put <source-file> <dest-dir>"));
                }
                upload_file(layer, actions[1], actions[2]).await
            }
            _ => {
                // Execute command
                let command = actions.join(" ");
                execute_command(layer, &command).await
            }
        }
    }
}

async fn run_interactive_shell(layer: &mut PktEncLayer) -> TshResult<()> {
    info!("Starting interactive shell");

    // Send shell mode
    layer.write(&[OperationMode::RunShell as u8]).await?;

    enable_raw_mode().map_err(|e| TshError::system(format!("Failed to enable raw mode: {}", e)))?;
    execute!(stdout(), EnterAlternateScreen)
        .map_err(|e| TshError::system(format!("Failed to enter alternate screen: {}", e)))?;

    let result = shell_loop(layer).await;

    // Cleanup
    disable_raw_mode()
        .map_err(|e| TshError::system(format!("Failed to disable raw mode: {}", e)))?;
    execute!(stdout(), LeaveAlternateScreen)
        .map_err(|e| TshError::system(format!("Failed to leave alternate screen: {}", e)))?;

    result
}

async fn shell_loop(layer: &mut PktEncLayer) -> TshResult<()> {
    let mut buffer = vec![0u8; BUFSIZE];

    // Create a simple keyboard handler that doesn't conflict with layer usage
    loop {
        // Check for keyboard input without async block
        if event::poll(std::time::Duration::from_millis(10)).unwrap_or(false) {
            if let Ok(Event::Key(key_event)) = event::read() {
                match key_event.code {
                    KeyCode::Char(c) => {
                        let _ = layer.write(&[c as u8]).await;
                    }
                    KeyCode::Enter => {
                        let _ = layer.write(b"\r\n").await;
                    }
                    KeyCode::Backspace => {
                        let _ = layer.write(&[8]).await;
                    }
                    KeyCode::Esc => {
                        return Ok(());
                    }
                    _ => {}
                }
            }
        }

        // Try to read from remote with timeout
        match tokio::time::timeout(
            std::time::Duration::from_millis(10),
            layer.read(&mut buffer),
        )
        .await
        {
            Ok(Ok(n)) => {
                if n == 0 {
                    break;
                }
                print!("{}", String::from_utf8_lossy(&buffer[..n]));
                std::io::stdout().flush().unwrap();
            }
            Ok(Err(TshError::ConnectionClosed)) => break,
            Ok(Err(e)) => {
                error!("Read error: {}", e);
                break;
            }
            Err(_) => {
                // Timeout - continue loop
                continue;
            }
        }
    }

    Ok(())
}

async fn download_file(layer: &mut PktEncLayer, source: &str, dest_dir: &str) -> TshResult<()> {
    info!("Downloading {} to {}", source, dest_dir);

    // Send get command
    layer.write(&[OperationMode::GetFile as u8]).await?;
    layer.write(source.as_bytes()).await?;
    layer.write(b"\0").await?;

    // Read file size
    let mut size_buf = vec![0u8; 8];
    layer.read(&mut size_buf).await?;
    let file_size = u64::from_le_bytes(size_buf.try_into().unwrap());

    // Create destination file
    let dest_path = Path::new(dest_dir).join(Path::new(source).file_name().unwrap());
    let mut dest_file = File::create(&dest_path)
        .await
        .map_err(|e| TshError::file_transfer(format!("Failed to create file: {}", e)))?;

    // Setup progress bar
    let pb = ProgressBar::new(file_size);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{bar:40.cyan/blue} {pos}/{len} {percent}% {eta}")
            .unwrap(),
    );

    // Download file with progress
    let mut total_read = 0u64;
    let mut buffer = vec![0u8; BUFSIZE];

    while total_read < file_size {
        let to_read = std::cmp::min(buffer.len(), (file_size - total_read) as usize);
        let n = layer.read(&mut buffer[..to_read]).await?;

        if n == 0 {
            break;
        }

        dest_file
            .write_all(&buffer[..n])
            .await
            .map_err(|e| TshError::file_transfer(format!("Failed to write to file: {}", e)))?;

        total_read += n as u64;
        pb.set_position(total_read);
    }

    pb.finish();
    println!("\nDone.");

    Ok(())
}

async fn upload_file(layer: &mut PktEncLayer, source: &str, dest_dir: &str) -> TshResult<()> {
    info!("Uploading {} to {}", source, dest_dir);

    // Open source file
    let mut source_file = File::open(source)
        .await
        .map_err(|e| TshError::file_transfer(format!("Failed to open file: {}", e)))?;

    let file_size = source_file
        .metadata()
        .await
        .map_err(|e| TshError::file_transfer(format!("Failed to get file metadata: {}", e)))?
        .len();

    // Send put command
    layer.write(&[OperationMode::PutFile as u8]).await?;
    layer.write(dest_dir.as_bytes()).await?;
    layer.write(b"\0").await?;

    // Send file size
    layer.write(&file_size.to_le_bytes()).await?;

    // Setup progress bar
    let pb = ProgressBar::new(file_size);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{bar:40.cyan/blue} {pos}/{len} {percent}% {eta}")
            .unwrap(),
    );

    // Upload file with progress
    let mut total_sent = 0u64;
    let mut buffer = vec![0u8; BUFSIZE];

    while total_sent < file_size {
        let n = source_file
            .read(&mut buffer)
            .await
            .map_err(|e| TshError::file_transfer(format!("Failed to read from file: {}", e)))?;

        if n == 0 {
            break;
        }

        layer.write(&buffer[..n]).await?;
        total_sent += n as u64;
        pb.set_position(total_sent);
    }

    pb.finish();
    println!("\nDone.");

    Ok(())
}

async fn execute_command(layer: &mut PktEncLayer, command: &str) -> TshResult<()> {
    info!("Executing command: {}", command);

    // Send command
    layer.write(command.as_bytes()).await?;
    layer.write(b"\0").await?;

    // Read and display output
    let mut buffer = vec![0u8; BUFSIZE];
    loop {
        match layer.read(&mut buffer).await {
            Ok(n) => {
                if n == 0 {
                    break;
                }
                print!("{}", String::from_utf8_lossy(&buffer[..n]));
            }
            Err(TshError::ConnectionClosed) => break,
            Err(e) => return Err(e),
        }
    }

    Ok(())
}
