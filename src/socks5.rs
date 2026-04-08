//! SOCKS5 proxy module (RFC 1928)
//!
//! Client side: local SOCKS5 listener → forwards through Noise tunnel to agent
//! Server side: receives target address → connects → relays data

use log::{error, info, warn};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

use crate::{
    constants::OperationMode,
    error::*,
    helpers::NoiseLayerExt,
    noise::{NoiseLayer, NoiseListener},
};

// SOCKS5 constants
const SOCKS_VERSION: u8 = 0x05;
const AUTH_NO_AUTH: u8 = 0x00;
const CMD_CONNECT: u8 = 0x01;
const ATYP_IPV4: u8 = 0x01;
const ATYP_DOMAIN: u8 = 0x03;
const ATYP_IPV6: u8 = 0x04;
const REP_SUCCESS: u8 = 0x00;
const REP_CONNECTION_REFUSED: u8 = 0x05;

/// Target address parsed from SOCKS5 CONNECT request
#[derive(Debug)]
pub(crate) struct TargetAddr {
    pub(crate) host: String,
    pub(crate) port: u16,
}

impl TargetAddr {
    /// Serialize to wire format: [host_len: u16][host_bytes][port: u16]
    pub(crate) fn to_bytes(&self) -> Vec<u8> {
        let host_bytes = self.host.as_bytes();
        let mut buf = Vec::with_capacity(2 + host_bytes.len() + 2);
        buf.extend_from_slice(&(host_bytes.len() as u16).to_be_bytes());
        buf.extend_from_slice(host_bytes);
        buf.extend_from_slice(&self.port.to_be_bytes());
        buf
    }

    /// Deserialize from wire format
    pub(crate) fn from_bytes(data: &[u8]) -> TshResult<Self> {
        if data.len() < 4 {
            return Err(TshError::protocol("Target address too short"));
        }
        let host_len = u16::from_be_bytes([data[0], data[1]]) as usize;
        if data.len() < 2 + host_len + 2 {
            return Err(TshError::protocol("Target address truncated"));
        }
        let host = std::str::from_utf8(&data[2..2 + host_len])
            .map_err(|_| TshError::protocol("Target host not valid UTF-8"))?
            .to_string();
        let port = u16::from_be_bytes([data[2 + host_len], data[2 + host_len + 1]]);
        Ok(Self { host, port })
    }
}

// ============================================================
// Client side: local SOCKS5 listener
// ============================================================

/// Start a local SOCKS5 proxy that tunnels through the Noise connection to the agent.
/// Each incoming SOCKS5 connection opens a new Noise session to the server.
pub async fn run_socks5_client(local_bind: &str, server_addr: &str, psk: &str) -> TshResult<()> {
    let listener = TcpListener::bind(local_bind).await.map_err(|e| {
        TshError::network(format!(
            "Failed to bind SOCKS5 listener on {local_bind}: {e}"
        ))
    })?;

    println!("SOCKS5 proxy listening on {local_bind}");
    println!("  Tunneling through agent at {server_addr}");
    println!("  Configure your tools: --proxy socks5://127.0.0.1:1080");

    loop {
        let (stream, peer) = listener
            .accept()
            .await
            .map_err(|e| TshError::network(format!("Failed to accept SOCKS5 connection: {e}")))?;

        let server_addr = server_addr.to_string();
        let psk = psk.to_string();

        tokio::spawn(async move {
            info!("SOCKS5 connection from {peer}");
            if let Err(e) = handle_socks5_client_connection(stream, &server_addr, &psk).await {
                warn!("SOCKS5 session error for {peer}: {e}");
            }
        });
    }
}

async fn handle_socks5_client_connection(
    mut stream: TcpStream,
    server_addr: &str,
    psk: &str,
) -> TshResult<()> {
    // 1. SOCKS5 handshake
    socks5_handshake(&mut stream).await?;

    // 2. Parse CONNECT request
    let target = socks5_read_connect(&mut stream).await?;
    info!("SOCKS5 CONNECT to {}:{}", target.host, target.port);

    // 3. Open Noise connection to agent
    let mut layer = NoiseLayer::connect(server_addr, psk).await?;

    // 4. Send Socks5 mode + target address
    let target_bytes = target.to_bytes();
    let mut data = Vec::with_capacity(1 + target_bytes.len());
    data.push(OperationMode::Socks5 as u8);
    data.extend_from_slice(&target_bytes);
    layer.write_all(&data).await?;

    // 5. Read server response (1 byte: 0=success, 1=failure)
    let mut resp = [0u8; 1];
    layer.read_exact(&mut resp).await?;

    if resp[0] != 0 {
        socks5_send_reply(&mut stream, REP_CONNECTION_REFUSED).await?;
        return Err(TshError::network(format!(
            "Agent failed to connect to {}:{}",
            target.host, target.port
        )));
    }

    // 6. Send SOCKS5 success reply to local client
    socks5_send_reply(&mut stream, REP_SUCCESS).await?;

    // 7. Relay data bidirectionally
    relay_data(&mut stream, &mut layer).await
}

// ============================================================
// Server (agent) side: receive target, connect, relay
// ============================================================

/// Handle a Socks5 proxy request from the client.
/// Called by server when it receives OperationMode::Socks5.
pub async fn handle_socks5_server(layer: &mut NoiseLayer, initial_data: &[u8]) -> TshResult<()> {
    // Parse target address from the data after mode byte
    let target = TargetAddr::from_bytes(initial_data)?;
    let addr = format!("{}:{}", target.host, target.port);
    info!("SOCKS5 proxy: connecting to {addr}");

    // Connect to target
    match TcpStream::connect(&addr).await {
        Ok(mut tcp_stream) => {
            // Send success to client
            layer.write_all(&[0u8]).await?;
            info!("SOCKS5 proxy: connected to {addr}");

            // Relay data
            relay_data_server(&mut tcp_stream, layer).await
        }
        Err(e) => {
            warn!("SOCKS5 proxy: failed to connect to {addr}: {e}");
            layer.write_all(&[1u8]).await?;
            Ok(())
        }
    }
}

// ============================================================
// SOCKS5 protocol helpers
// ============================================================

/// SOCKS5 initial handshake: client sends version + auth methods, we reply no-auth
async fn socks5_handshake(stream: &mut TcpStream) -> TshResult<()> {
    // Read: [version(1)][nmethods(1)][methods(nmethods)]
    let mut header = [0u8; 2];
    stream.read_exact(&mut header).await?;

    if header[0] != SOCKS_VERSION {
        return Err(TshError::protocol(format!(
            "Unsupported SOCKS version: {}",
            header[0]
        )));
    }

    let nmethods = header[1] as usize;
    let mut methods = vec![0u8; nmethods];
    stream.read_exact(&mut methods).await?;

    // Reply: no authentication required
    stream.write_all(&[SOCKS_VERSION, AUTH_NO_AUTH]).await?;
    Ok(())
}

/// Read SOCKS5 CONNECT request and extract target address
async fn socks5_read_connect(stream: &mut TcpStream) -> TshResult<TargetAddr> {
    // [version(1)][cmd(1)][rsv(1)][atyp(1)]
    let mut header = [0u8; 4];
    stream.read_exact(&mut header).await?;

    if header[0] != SOCKS_VERSION {
        return Err(TshError::protocol("Invalid SOCKS5 request version"));
    }
    if header[1] != CMD_CONNECT {
        return Err(TshError::protocol(format!(
            "Unsupported SOCKS5 command: {}",
            header[1]
        )));
    }

    let atyp = header[3];
    let host = match atyp {
        ATYP_IPV4 => {
            let mut addr = [0u8; 4];
            stream.read_exact(&mut addr).await?;
            format!("{}.{}.{}.{}", addr[0], addr[1], addr[2], addr[3])
        }
        ATYP_DOMAIN => {
            let mut len_buf = [0u8; 1];
            stream.read_exact(&mut len_buf).await?;
            let mut domain = vec![0u8; len_buf[0] as usize];
            stream.read_exact(&mut domain).await?;
            String::from_utf8(domain).map_err(|_| TshError::protocol("Invalid domain name"))?
        }
        ATYP_IPV6 => {
            let mut addr = [0u8; 16];
            stream.read_exact(&mut addr).await?;
            let segments: Vec<String> = addr
                .chunks(2)
                .map(|c| format!("{:02x}{:02x}", c[0], c[1]))
                .collect();
            segments.join(":")
        }
        _ => {
            return Err(TshError::protocol(format!(
                "Unsupported address type: {atyp}"
            )));
        }
    };

    // Read port (2 bytes, big-endian)
    let mut port_buf = [0u8; 2];
    stream.read_exact(&mut port_buf).await?;
    let port = u16::from_be_bytes(port_buf);

    Ok(TargetAddr { host, port })
}

/// Send SOCKS5 reply
async fn socks5_send_reply(stream: &mut TcpStream, reply: u8) -> TshResult<()> {
    // [version(1)][reply(1)][rsv(1)][atyp(1)][bind_addr(4)][bind_port(2)]
    let response = [SOCKS_VERSION, reply, 0x00, ATYP_IPV4, 0, 0, 0, 0, 0, 0];
    stream.write_all(&response).await?;
    Ok(())
}

// ============================================================
// Bidirectional relay
// ============================================================

/// Relay between local TCP stream and Noise layer (client side)
async fn relay_data(stream: &mut TcpStream, layer: &mut NoiseLayer) -> TshResult<()> {
    let mut tcp_buf = vec![0u8; 8192];
    let mut noise_buf = vec![0u8; 8192];

    loop {
        tokio::select! {
            // TCP → Noise (local app sends data to remote)
            result = stream.read(&mut tcp_buf) => {
                match result {
                    Ok(0) => break,
                    Ok(n) => layer.write_all(&tcp_buf[..n]).await?,
                    Err(e) => {
                        error!("SOCKS5 local read error: {e}");
                        break;
                    }
                }
            }
            // Noise → TCP (remote sends data back to local app)
            result = layer.read(&mut noise_buf) => {
                match result {
                    Ok(0) => break,
                    Ok(n) => stream.write_all(&noise_buf[..n]).await?,
                    Err(e) => {
                        error!("SOCKS5 tunnel read error: {e}");
                        break;
                    }
                }
            }
        }
    }

    Ok(())
}

/// Relay between target TCP stream and Noise layer (server/agent side)
async fn relay_data_server(stream: &mut TcpStream, layer: &mut NoiseLayer) -> TshResult<()> {
    let mut tcp_buf = vec![0u8; 8192];
    let mut noise_buf = vec![0u8; 8192];

    loop {
        tokio::select! {
            // Target → Noise (target sends data back through tunnel)
            result = stream.read(&mut tcp_buf) => {
                match result {
                    Ok(0) => break,
                    Ok(n) => layer.write_all(&tcp_buf[..n]).await?,
                    Err(e) => {
                        error!("SOCKS5 target read error: {e}");
                        break;
                    }
                }
            }
            // Noise → Target (client sends data to target)
            result = layer.read(&mut noise_buf) => {
                match result {
                    Ok(0) => break,
                    Ok(n) => stream.write_all(&noise_buf[..n]).await?,
                    Err(e) => {
                        error!("SOCKS5 tunnel read error: {e}");
                        break;
                    }
                }
            }
        }
    }

    Ok(())
}

// ============================================================
// Standalone SOCKS5 client runner (for use from main)
// ============================================================

/// Run SOCKS5 proxy with a pre-existing NoiseListener (for connect-back mode)
/// Not implemented yet — SOCKS5 currently only works in direct mode.
pub async fn run_socks5_client_with_listener(
    local_bind: &str,
    _listener: &NoiseListener,
) -> TshResult<()> {
    Err(TshError::protocol(format!(
        "SOCKS5 in connect-back mode is not yet supported (bind: {local_bind})"
    )))
}
