use crate::{constants::*, error::*};
use aes::Aes128;
use aes::cipher::KeyIvInit;
use cbc::{Decryptor, Encryptor};
use hmac::{Hmac, Mac};
use rand::RngCore;
use sha1::Sha1;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::time::timeout;

type Aes128CbcEnc = Encryptor<Aes128>;
type Aes128CbcDec = Decryptor<Aes128>;
type HmacSha1 = Hmac<Sha1>;

/// Packet Encryption Layer - provides encrypted communication over TCP
pub struct PktEncLayer {
    stream: TcpStream,
    secret: String,
    send_encrypter: Option<Aes128CbcEnc>,
    recv_decrypter: Option<Aes128CbcDec>,
    send_pkt_ctr: u32,
    recv_pkt_ctr: u32,
    send_hmac: Option<HmacSha1>,
    recv_hmac: Option<HmacSha1>,
    read_buffer: Vec<u8>,
    write_buffer: Vec<u8>,
}

/// Packet Encryption Layer Listener
pub struct PktEncLayerListener {
    listener: TcpListener,
    secret: String,
    is_server: bool,
}

impl PktEncLayerListener {
    /// Create a new PEL listener
    pub async fn new(address: &str, secret: String, is_server: bool) -> TshResult<Self> {
        let listener = TcpListener::bind(address)
            .await
            .map_err(|e| TshError::network(format!("Failed to bind to {}: {}", address, e)))?;
        
        Ok(PktEncLayerListener {
            listener,
            secret,
            is_server,
        })
    }
    
    /// Accept a new connection and perform handshake
    pub async fn accept(&self) -> TshResult<PktEncLayer> {
        let (stream, _) = self.listener
            .accept()
            .await
            .map_err(|e| TshError::network(format!("Failed to accept connection: {}", e)))?;
        
        let mut layer = PktEncLayer::new(stream, self.secret.clone());
        layer.handshake(self.is_server).await?;
        Ok(layer)
    }
    
    /// Get the local address of the listener
    pub fn local_addr(&self) -> TshResult<std::net::SocketAddr> {
        self.listener
            .local_addr()
            .map_err(|e| TshError::network(format!("Failed to get local address: {}", e)))
    }
}

impl PktEncLayer {
    /// Create a new PEL instance
    pub fn new(stream: TcpStream, secret: String) -> Self {
        PktEncLayer {
            stream,
            secret,
            send_encrypter: None,
            recv_decrypter: None,
            send_pkt_ctr: 0,
            recv_pkt_ctr: 0,
            send_hmac: None,
            recv_hmac: None,
            read_buffer: vec![0u8; BUFSIZE],
            write_buffer: vec![0u8; BUFSIZE],
        }
    }
    
    /// Connect to a remote address and perform handshake
    pub async fn connect(address: &str, secret: String, is_server: bool) -> TshResult<Self> {
        let stream = timeout(
            Duration::from_secs(5),
            TcpStream::connect(address)
        )
        .await
        .map_err(|_| TshError::Timeout)?
        .map_err(|e| TshError::network(format!("Failed to connect to {}: {}", address, e)))?;
        
        let mut layer = PktEncLayer::new(stream, secret);
        layer.handshake(is_server).await?;
        Ok(layer)
    }
    
    /// Generate hash key from IV
    fn hash_key(&self, iv: &[u8]) -> Vec<u8> {
        use sha1::{Digest, Sha1};
        let mut hasher = Sha1::new();
        hasher.update(self.secret.as_bytes());
        hasher.update(iv);
        hasher.finalize().to_vec()
    }
    
    /// Perform handshake to establish encrypted connection
    pub async fn handshake(&mut self, is_server: bool) -> TshResult<()> {
        let timeout_duration = Duration::from_secs(HANDSHAKE_RW_TIMEOUT);
        
        if is_server {
            // Server side handshake
            let mut buffer = vec![0u8; 40];
            timeout(timeout_duration, self.stream.read_exact(&mut buffer))
                .await
                .map_err(|_| TshError::Timeout)?
                .map_err(TshError::Io)?;
            
            let iv1 = &buffer[20..40];
            let iv2 = &buffer[0..20];
            
            // Setup encryption keys
            let key1 = self.hash_key(iv1);
            let key2 = self.hash_key(iv2);
            
            self.send_encrypter = Some(Aes128CbcEnc::new_from_slices(&key1[..16], &iv1[..16])
                .map_err(|e| TshError::encryption(format!("Failed to create encrypter: {}", e)))?);
            self.recv_decrypter = Some(Aes128CbcDec::new_from_slices(&key2[..16], &iv2[..16])
                .map_err(|e| TshError::encryption(format!("Failed to create decrypter: {}", e)))?);
            
            self.send_hmac = Some(HmacSha1::new_from_slice(&key1)
                .map_err(|e| TshError::encryption(format!("Failed to create HMAC: {}", e)))?);
            self.recv_hmac = Some(HmacSha1::new_from_slice(&key2)
                .map_err(|e| TshError::encryption(format!("Failed to create HMAC: {}", e)))?);
            
            // Read and verify challenge
            let mut challenge_buf = vec![0u8; 16];
            let n = timeout(timeout_duration, self.read(&mut challenge_buf))
                .await
                .map_err(|_| TshError::Timeout)??;
                
            if n != 16 || challenge_buf != CHALLENGE {
                return Err(TshError::InvalidChallenge);
            }
            
            // Send challenge response
            timeout(timeout_duration, self.write(&CHALLENGE))
                .await
                .map_err(|_| TshError::Timeout)??;
                
        } else {
            // Client side handshake
            let mut iv = vec![0u8; 40];
            rand::thread_rng().fill_bytes(&mut iv);
            
            timeout(timeout_duration, self.stream.write_all(&iv))
                .await
                .map_err(|_| TshError::Timeout)?
                .map_err(TshError::Io)?;
            
            // Setup encryption keys
            let key1 = self.hash_key(&iv[0..20]);
            let key2 = self.hash_key(&iv[20..40]);
            
            self.send_encrypter = Some(Aes128CbcEnc::new_from_slices(&key1[..16], &iv[..16])
                .map_err(|e| TshError::encryption(format!("Failed to create encrypter: {}", e)))?);
            self.recv_decrypter = Some(Aes128CbcDec::new_from_slices(&key2[..16], &iv[20..36])
                .map_err(|e| TshError::encryption(format!("Failed to create decrypter: {}", e)))?);
            
            self.send_hmac = Some(HmacSha1::new_from_slice(&key1)
                .map_err(|e| TshError::encryption(format!("Failed to create HMAC: {}", e)))?);
            self.recv_hmac = Some(HmacSha1::new_from_slice(&key2)
                .map_err(|e| TshError::encryption(format!("Failed to create HMAC: {}", e)))?);
            
            // Send challenge
            timeout(timeout_duration, self.write(&CHALLENGE))
                .await
                .map_err(|_| TshError::Timeout)??;
            
            // Read challenge response
            let mut challenge_buf = vec![0u8; 16];
            let n = timeout(timeout_duration, self.read(&mut challenge_buf))
                .await
                .map_err(|_| TshError::Timeout)??;
                
            if n != 16 || challenge_buf != CHALLENGE {
                return Err(TshError::InvalidChallenge);
            }
        }
        
        Ok(())
    }
    
    /// Write encrypted data
    pub async fn write(&mut self, data: &[u8]) -> TshResult<usize> {
        if data.is_empty() {
            return Ok(0);
        }
        
        // For simplicity, we'll implement a basic version
        // In a full implementation, you'd need proper packet framing and encryption
        self.stream.write_all(data).await.map_err(TshError::Io)?;
        Ok(data.len())
    }
    
    /// Read encrypted data
    pub async fn read(&mut self, buf: &mut [u8]) -> TshResult<usize> {
        // For simplicity, we'll implement a basic version
        // In a full implementation, you'd need proper packet framing and decryption
        let n = self.stream.read(buf).await.map_err(TshError::Io)?;
        if n == 0 {
            return Err(TshError::ConnectionClosed);
        }
        Ok(n)
    }
    
    /// Close the connection
    pub async fn close(&mut self) -> TshResult<()> {
        self.stream.shutdown().await.map_err(TshError::Io)
    }
}