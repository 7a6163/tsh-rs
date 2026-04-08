use tokio::io::{AsyncReadExt, AsyncWriteExt};

/// Test that WsByteStream correctly adapts WebSocket to AsyncRead/AsyncWrite.
/// Uses a real in-process WebSocket server/client pair.
#[tokio::test]
async fn test_ws_byte_stream_round_trip() {
    use crate::c2_https::WsByteStream;
    use tokio::net::TcpListener;
    use tokio_tungstenite::{accept_async, connect_async};

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    // Server task: accept WS, read data, echo it back
    let server = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        let ws = accept_async(stream).await.unwrap();
        let mut byte_stream = WsByteStream::new(ws);

        let mut buf = vec![0u8; 1024];
        let n = byte_stream.read(&mut buf).await.unwrap();
        byte_stream.write_all(&buf[..n]).await.unwrap();
        byte_stream.flush().await.unwrap();
    });

    // Client: connect WS, send data, read echo
    let url = format!("ws://127.0.0.1:{}", addr.port());
    let (ws, _) = connect_async(&url).await.unwrap();
    let mut byte_stream = WsByteStream::new(ws);

    let payload = b"hello from ws test";
    byte_stream.write_all(payload).await.unwrap();
    byte_stream.flush().await.unwrap();

    let mut buf = vec![0u8; 1024];
    let n = byte_stream.read(&mut buf).await.unwrap();
    assert_eq!(&buf[..n], payload);

    server.await.unwrap();
}

/// Test that multiple messages can be sent/received sequentially
#[tokio::test]
async fn test_ws_byte_stream_multiple_messages() {
    use crate::c2_https::WsByteStream;
    use tokio::net::TcpListener;
    use tokio_tungstenite::{accept_async, connect_async};

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let server = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        let ws = accept_async(stream).await.unwrap();
        let mut s = WsByteStream::new(ws);

        for _ in 0..3 {
            let mut buf = vec![0u8; 1024];
            let n = s.read(&mut buf).await.unwrap();
            s.write_all(&buf[..n]).await.unwrap();
            s.flush().await.unwrap();
        }
    });

    let url = format!("ws://127.0.0.1:{}", addr.port());
    let (ws, _) = connect_async(&url).await.unwrap();
    let mut c = WsByteStream::new(ws);

    for i in 0..3u8 {
        let msg = vec![i; 100];
        c.write_all(&msg).await.unwrap();
        c.flush().await.unwrap();

        let mut buf = vec![0u8; 1024];
        let n = c.read(&mut buf).await.unwrap();
        assert_eq!(&buf[..n], &msg[..]);
    }

    server.await.unwrap();
}

/// Test that WsByteStream handles large payloads correctly
#[tokio::test]
async fn test_ws_byte_stream_large_payload() {
    use crate::c2_https::WsByteStream;
    use tokio::net::TcpListener;
    use tokio_tungstenite::{accept_async, connect_async};

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let server = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        let ws = accept_async(stream).await.unwrap();
        let mut s = WsByteStream::new(ws);

        let mut buf = vec![0u8; 65536];
        let mut total = 0;
        while total < 32768 {
            let n = s.read(&mut buf[total..]).await.unwrap();
            if n == 0 {
                break;
            }
            total += n;
        }
        // Echo back all received data
        s.write_all(&buf[..total]).await.unwrap();
        s.flush().await.unwrap();
    });

    let url = format!("ws://127.0.0.1:{}", addr.port());
    let (ws, _) = connect_async(&url).await.unwrap();
    let mut c = WsByteStream::new(ws);

    // Send 32KB of data
    let payload: Vec<u8> = (0..32768u32).map(|i| (i % 256) as u8).collect();
    c.write_all(&payload).await.unwrap();
    c.flush().await.unwrap();

    let mut received = vec![0u8; 65536];
    let mut total = 0;
    while total < payload.len() {
        let n = c.read(&mut received[total..]).await.unwrap();
        if n == 0 {
            break;
        }
        total += n;
    }
    assert_eq!(total, payload.len());
    assert_eq!(&received[..total], &payload[..]);

    server.await.unwrap();
}
