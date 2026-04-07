use crate::pty::Pty;

#[tokio::test]
async fn test_pty_creation() {
    let result = Pty::new();
    assert!(result.is_ok(), "Failed to create PTY: {:?}", result.err());
}

#[tokio::test]
async fn test_pty_write_and_read() {
    let pty = Pty::new().unwrap();

    // Write a command to the PTY
    let written = pty.write(b"echo hello\n").await;
    assert!(written.is_ok(), "PTY write failed: {:?}", written.err());
    assert!(written.unwrap() > 0);

    // Read output from the PTY (should get something back)
    let mut buf = vec![0u8; 4096];
    let read_result = pty.read(&mut buf).await;
    assert!(read_result.is_ok(), "PTY read failed: {:?}", read_result.err());
    assert!(read_result.unwrap() > 0);
}

#[tokio::test]
async fn test_pty_multiple_writes() {
    let pty = Pty::new().unwrap();

    // Write multiple times — verifies writer is not consumed after first use
    for i in 0..5 {
        let cmd = format!("echo test_{i}\n");
        let result = pty.write(cmd.as_bytes()).await;
        assert!(
            result.is_ok(),
            "PTY write #{i} failed: {:?}",
            result.err()
        );
    }
}

#[tokio::test]
async fn test_pty_resize() {
    let pty = Pty::new().unwrap();

    let result = pty.resize(40, 120).await;
    assert!(result.is_ok(), "PTY resize failed: {:?}", result.err());

    // Resize to a different size
    let result = pty.resize(24, 80).await;
    assert!(result.is_ok(), "PTY resize failed: {:?}", result.err());
}

#[tokio::test]
async fn test_pty_write_empty_data() {
    let pty = Pty::new().unwrap();

    // Writing empty data should not panic
    let result = pty.write(b"").await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_pty_concurrent_read_write() {
    let pty = Pty::new().unwrap();

    // Spawn a writer task
    let pty_ref = &pty;

    let write_handle = tokio::spawn({
        let write_result = pty_ref.write(b"echo concurrent\n").await;
        async move { write_result }
    });

    // Give it a moment to process
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // Read from PTY
    let mut buf = vec![0u8; 4096];
    let read_result = pty.read(&mut buf).await;
    assert!(read_result.is_ok());

    let _ = write_handle.await;
}
