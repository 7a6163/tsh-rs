use crate::{constants::*, error::*};
use snow::{Builder, HandshakeState, TransportState};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::time::timeout;

// Noise protocol pattern: XX
// -> e
// <- e, ee, s, es
// -> s, se
const NOISE_PATTERN: &str = "Noise_XX_25519_ChaChaPoly_BLAKE2s";

// Maximum message size for Noise (64KB - overhead)
const MAX_MESSAGE_SIZE: usize = 65535 - 16;

/// Noise Protocol Layer - provides encrypted communication over TCP
pub struct NoiseLayer {
    stream: TcpStream,
    transport: TransportState,
}

/// Noise Protocol Listener
pub struct NoiseListener {
    listener: TcpListener,
    static_key: Vec<u8>,
}

impl NoiseListener {
    /// Create a new Noise listener
    pub async fn new(address: &str) -> TshResult<Self> {
        let listener = TcpListener::bind(address)
            .await
            .map_err(|e| TshError::network(format!("Failed to bind to {address}: {e}")))?;

        // Generate static keypair for the server
        let builder = Builder::new(NOISE_PATTERN.parse().unwrap());
        let keypair = builder
            .generate_keypair()
            .map_err(|e| TshError::encryption(format!("Failed to generate keypair: {e}")))?;

        Ok(NoiseListener {
            listener,
            static_key: keypair.private,
        })
    }

    /// Accept a new connection and perform Noise handshake
    pub async fn accept(&self) -> TshResult<NoiseLayer> {
        let (stream, addr) = self
            .listener
            .accept()
            .await
            .map_err(|e| TshError::network(format!("Failed to accept connection: {e}")))?;

        log::info!("Accepted connection from: {addr}");

        // Create responder
        let builder = Builder::new(NOISE_PATTERN.parse().unwrap());
        let handshake = builder
            .local_private_key(&self.static_key)
            .build_responder()
            .map_err(|e| TshError::encryption(format!("Failed to build responder: {e}")))?;

        // Perform handshake
        let transport = perform_handshake_responder(stream, handshake).await?;
        Ok(transport)
    }

    /// Get the local address of the listener
    pub fn local_addr(&self) -> TshResult<std::net::SocketAddr> {
        self.listener
            .local_addr()
            .map_err(|e| TshError::network(format!("Failed to get local address: {e}")))
    }

    /// Get the server's public key (for client pre-sharing)
    pub fn public_key(&self) -> TshResult<Vec<u8>> {
        let builder = Builder::new(NOISE_PATTERN.parse().unwrap());
        let keypair = builder.generate_keypair().unwrap();
        Ok(keypair.public)
    }
}

impl NoiseLayer {
    /// Connect to a remote address and perform Noise handshake
    pub async fn connect(address: &str) -> TshResult<Self> {
        let stream = timeout(Duration::from_secs(5), TcpStream::connect(address))
            .await
            .map_err(|_| TshError::Timeout)?
            .map_err(|e| TshError::network(format!("Failed to connect to {address}: {e}")))?;

        // Create initiator
        let builder = Builder::new(NOISE_PATTERN.parse().unwrap());
        let handshake = builder
            .build_initiator()
            .map_err(|e| TshError::encryption(format!("Failed to build initiator: {e}")))?;

        // Perform handshake
        let transport = perform_handshake_initiator(stream, handshake).await?;
        Ok(transport)
    }

    /// Write encrypted data
    pub async fn write(&mut self, data: &[u8]) -> TshResult<usize> {
        if data.is_empty() {
            return Ok(0);
        }

        // Split data into chunks if necessary
        let mut written = 0;
        for chunk in data.chunks(MAX_MESSAGE_SIZE) {
            let mut buf = vec![0u8; MAX_MESSAGE_SIZE + 16]; // Extra space for tag
            let len = self
                .transport
                .write_message(chunk, &mut buf)
                .map_err(|e| TshError::encryption(format!("Failed to encrypt: {e}")))?;

            // Write length prefix (4 bytes, big-endian)
            let len_bytes = (len as u32).to_be_bytes();
            self.stream
                .write_all(&len_bytes)
                .await
                .map_err(TshError::Io)?;

            // Write encrypted message
            self.stream
                .write_all(&buf[..len])
                .await
                .map_err(TshError::Io)?;
            written += chunk.len();
        }

        Ok(written)
    }

    /// Read encrypted data
    pub async fn read(&mut self, buf: &mut [u8]) -> TshResult<usize> {
        // Read length prefix
        let mut len_bytes = [0u8; 4];
        self.stream.read_exact(&mut len_bytes).await.map_err(|e| {
            if e.kind() == std::io::ErrorKind::UnexpectedEof {
                TshError::ConnectionClosed
            } else {
                TshError::Io(e)
            }
        })?;

        let msg_len = u32::from_be_bytes(len_bytes) as usize;
        if msg_len > MAX_MESSAGE_SIZE + 16 {
            return Err(TshError::encryption("Message too large"));
        }

        // Read encrypted message
        let mut encrypted = vec![0u8; msg_len];
        self.stream
            .read_exact(&mut encrypted)
            .await
            .map_err(TshError::Io)?;

        // Decrypt
        let len = self
            .transport
            .read_message(&encrypted, buf)
            .map_err(|e| TshError::encryption(format!("Failed to decrypt: {e}")))?;

        Ok(len)
    }

    /// Close the connection
    pub async fn close(&mut self) -> TshResult<()> {
        self.stream.shutdown().await.map_err(TshError::Io)
    }

    /// Get remote peer's static public key (after handshake)
    pub fn remote_public_key(&self) -> Option<Vec<u8>> {
        self.transport.get_remote_static().map(|k| k.to_vec())
    }
}

/// Perform Noise handshake as initiator
pub async fn perform_handshake_initiator(
    mut stream: TcpStream,
    mut handshake: HandshakeState,
) -> TshResult<NoiseLayer> {
    let timeout_duration = Duration::from_secs(HANDSHAKE_RW_TIMEOUT);
    let mut buf = vec![0u8; 65535];
    let mut msg = vec![0u8; 65535];

    // -> e
    let len = handshake
        .write_message(&[], &mut msg)
        .map_err(|e| TshError::encryption(format!("Handshake write failed: {e}")))?;

    timeout(timeout_duration, stream.write_all(&msg[..len]))
        .await
        .map_err(|_| TshError::Timeout)?
        .map_err(TshError::Io)?;

    // <- e, ee, s, es
    let n = timeout(timeout_duration, stream.read(&mut buf))
        .await
        .map_err(|_| TshError::Timeout)?
        .map_err(TshError::Io)?;

    let _len = handshake
        .read_message(&buf[..n], &mut msg)
        .map_err(|e| TshError::encryption(format!("Handshake read failed: {e}")))?;

    // -> s, se
    let len = handshake
        .write_message(&[], &mut msg)
        .map_err(|e| TshError::encryption(format!("Handshake write failed: {e}")))?;

    timeout(timeout_duration, stream.write_all(&msg[..len]))
        .await
        .map_err(|_| TshError::Timeout)?
        .map_err(TshError::Io)?;

    // Convert to transport mode
    let transport = handshake
        .into_transport_mode()
        .map_err(|e| TshError::encryption(format!("Failed to enter transport mode: {e}")))?;

    Ok(NoiseLayer { stream, transport })
}

/// Perform Noise handshake as responder
pub async fn perform_handshake_responder(
    mut stream: TcpStream,
    mut handshake: HandshakeState,
) -> TshResult<NoiseLayer> {
    let timeout_duration = Duration::from_secs(HANDSHAKE_RW_TIMEOUT);
    let mut buf = vec![0u8; 65535];
    let mut msg = vec![0u8; 65535];

    // -> e
    let n = timeout(timeout_duration, stream.read(&mut buf))
        .await
        .map_err(|_| TshError::Timeout)?
        .map_err(TshError::Io)?;

    let _len = handshake
        .read_message(&buf[..n], &mut msg)
        .map_err(|e| TshError::encryption(format!("Handshake read failed: {e}")))?;

    // <- e, ee, s, es
    let len = handshake
        .write_message(&[], &mut msg)
        .map_err(|e| TshError::encryption(format!("Handshake write failed: {e}")))?;

    timeout(timeout_duration, stream.write_all(&msg[..len]))
        .await
        .map_err(|_| TshError::Timeout)?
        .map_err(TshError::Io)?;

    // -> s, se
    let n = timeout(timeout_duration, stream.read(&mut buf))
        .await
        .map_err(|_| TshError::Timeout)?
        .map_err(TshError::Io)?;

    let _len = handshake
        .read_message(&buf[..n], &mut msg)
        .map_err(|e| TshError::encryption(format!("Handshake read failed: {e}")))?;

    // Convert to transport mode
    let transport = handshake
        .into_transport_mode()
        .map_err(|e| TshError::encryption(format!("Failed to enter transport mode: {e}")))?;

    Ok(NoiseLayer { stream, transport })
}

/// Helper function to derive key from password
pub fn derive_key_from_password(password: &str, salt: &[u8]) -> Vec<u8> {
    use sha2::{Digest, Sha256};

    let mut hasher = Sha256::new();
    hasher.update(password.as_bytes());
    hasher.update(salt);
    hasher.finalize().to_vec()
}
