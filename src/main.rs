use clap::{Arg, Command};

// Import our library modules
use tsh_rs::{client, error::*, server};

#[tokio::main]
async fn main() -> TshResult<()> {
    env_logger::init();

    let matches = Command::new("tsh")
        .version(env!("CARGO_PKG_VERSION"))
        .author("Zac")
        .about("Tiny Shell - Secure remote shell access tool with Noise Protocol + PSK")
        .subcommand_required(true)
        .arg_required_else_help(true)
        .subcommand(
            Command::new("server")
                .about("Run as server (daemon)")
                .arg(
                    Arg::new("port")
                        .short('p')
                        .long("port")
                        .value_name("PORT")
                        .help("Port number")
                        .default_value("1234"),
                )
                .arg(
                    Arg::new("psk")
                        .long("psk")
                        .value_name("PSK")
                        .help("Pre-shared key for authentication")
                        .required(true),
                )
                .arg(
                    Arg::new("connect-back")
                        .short('c')
                        .long("connect-back")
                        .value_name("HOST")
                        .help("Connect back to client host"),
                )
                .arg(
                    Arg::new("delay")
                        .short('d')
                        .long("delay")
                        .value_name("SECONDS")
                        .help("Connect-back delay in seconds")
                        .default_value("5"),
                ),
        )
        .subcommand(
            Command::new("client")
                .about("Run as client")
                .arg(
                    Arg::new("port")
                        .short('p')
                        .long("port")
                        .value_name("PORT")
                        .help("Port number")
                        .default_value("1234"),
                )
                .arg(
                    Arg::new("psk")
                        .long("psk")
                        .value_name("PSK")
                        .help("Pre-shared key for authentication")
                        .required(true),
                )
                .arg(
                    Arg::new("host")
                        .value_name("HOST")
                        .help("Target hostname or 'cb' for connect-back mode")
                        .required(true),
                )
                .arg(
                    Arg::new("action")
                        .value_name("ACTION")
                        .help("Action to perform (get:remote:local, put:local:remote, or command)")
                        .num_args(0..),
                ),
        )
        .get_matches();

    match matches.subcommand() {
        Some(("server", sub_matches)) => {
            let port: u16 = sub_matches
                .get_one::<String>("port")
                .unwrap()
                .parse()
                .map_err(|_| TshError::protocol("Invalid port number".to_string()))?;

            let psk = sub_matches.get_one::<String>("psk").unwrap();

            println!("🚀 tsh-rs v{} - Server Mode", env!("CARGO_PKG_VERSION"));
            println!("🔐 PSK: {}***", &psk[..4.min(psk.len())]);

            if let Some(host) = sub_matches.get_one::<String>("connect-back") {
                let delay: u64 = sub_matches
                    .get_one::<String>("delay")
                    .unwrap()
                    .parse()
                    .map_err(|_| TshError::protocol("Invalid delay".to_string()))?;

                println!("📡 Connect-back mode: {host} every {delay}s");
                run_connect_back_mode(host, port, delay, psk).await
            } else {
                println!("📡 Listen mode on port {port}");
                run_listen_mode(port, psk).await
            }
        }
        Some(("client", sub_matches)) => {
            let port: u16 = sub_matches
                .get_one::<String>("port")
                .unwrap()
                .parse()
                .map_err(|_| TshError::protocol("Invalid port number".to_string()))?;

            let psk = sub_matches.get_one::<String>("psk").unwrap();
            let host = sub_matches.get_one::<String>("host").unwrap();

            let actions: Vec<&str> = sub_matches
                .get_many::<String>("action")
                .map(|vals| vals.map(|s| s.as_str()).collect())
                .unwrap_or_default();

            println!("🚀 tsh-rs v{} - Client Mode", env!("CARGO_PKG_VERSION"));
            println!("🔐 PSK: {}***", &psk[..4.min(psk.len())]);

            if host == "cb" {
                println!("📡 Connect-back mode on port {port}");
                run_connect_back_client(port, actions, psk).await
            } else {
                println!("📡 Connecting to {host}:{port}");
                run_direct_client(host, port, actions, psk).await
            }
        }
        _ => unreachable!(),
    }
}

async fn run_listen_mode(port: u16, psk: &str) -> TshResult<()> {
    server::run_listen_mode(port, psk).await
}

async fn run_connect_back_mode(host: &str, port: u16, delay: u64, psk: &str) -> TshResult<()> {
    server::run_connect_back_mode(host, port, delay, psk).await
}

async fn run_direct_client(host: &str, port: u16, actions: Vec<&str>, psk: &str) -> TshResult<()> {
    client::handle_direct_connection(host, port, actions, psk).await
}

async fn run_connect_back_client(port: u16, actions: Vec<&str>, psk: &str) -> TshResult<()> {
    client::handle_connect_back_mode(port, actions, psk).await
}
