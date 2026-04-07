use crate::constants::OperationMode;
use crate::helpers::NoiseLayerExt;
use crate::noise::{NoiseLayer, NoiseListener};
use crate::server;

// ─── Path validation tests ───────────────────────────────────────────────────

#[test]
fn test_validate_file_path_rejects_absolute_path() {
    #[cfg(unix)]
    let path = b"/etc/passwd" as &[u8];
    #[cfg(windows)]
    let path = b"C:\\Windows\\System32\\config" as &[u8];

    let result = server::validate_file_path(path);
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("Absolute paths"), "Got: {err}");
}

#[test]
fn test_validate_file_path_rejects_traversal() {
    let result = server::validate_file_path(b"../../../etc/passwd");
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("traversal"), "Got: {err}");
}

#[test]
fn test_validate_file_path_rejects_mid_traversal() {
    let result = server::validate_file_path(b"foo/../../etc/passwd");
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("traversal"), "Got: {err}");
}

#[test]
fn test_validate_file_path_accepts_relative() {
    let result = server::validate_file_path(b"data/file.txt");
    assert!(result.is_ok());
    let path = result.unwrap();
    assert!(path.ends_with("data/file.txt"));
}

#[test]
fn test_validate_file_path_accepts_simple_filename() {
    let result = server::validate_file_path(b"readme.md");
    assert!(result.is_ok());
    let path = result.unwrap();
    assert!(path.ends_with("readme.md"));
}

#[test]
fn test_validate_file_path_rejects_invalid_utf8() {
    let result = server::validate_file_path(&[0xFF, 0xFE, 0x00]);
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("UTF-8"), "Got: {err}");
}

#[test]
fn test_extract_and_validate_path_null_terminated() {
    let data = b"data/file.txt\0extra";
    let result = server::extract_and_validate_path(data);
    assert!(result.is_ok());
    let path = result.unwrap();
    assert!(path.ends_with("data/file.txt"));
}

#[test]
fn test_extract_and_validate_path_no_null_terminator() {
    let data = b"data/file.txt";
    let result = server::extract_and_validate_path(data);
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("null-terminated"), "Got: {err}");
}

#[test]
fn test_extract_and_validate_path_rejects_absolute() {
    #[cfg(unix)]
    let data = b"/etc/passwd\0" as &[u8];
    #[cfg(windows)]
    let data = b"C:\\Windows\\System32\0" as &[u8];

    let result = server::extract_and_validate_path(data);
    assert!(result.is_err());
}

// ─── Operation mode dispatch tests ──────────────────────────────────────────

#[tokio::test]
async fn test_server_rejects_invalid_operation_mode() {
    let psk = "test_invalid_mode";

    let listener = NoiseListener::new("127.0.0.1:0", psk).await.unwrap();
    let addr = listener.local_addr().unwrap();

    let server_task = tokio::spawn(async move {
        let mut layer = listener.accept().await.unwrap();

        // Read the operation mode byte
        let mut buffer = vec![0u8; 8192];
        let n = layer.read(&mut buffer).await.unwrap();
        assert!(n > 0);

        // Try to parse it — should fail for invalid mode
        OperationMode::try_from(buffer[0])
    });

    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    let mut client = NoiseLayer::connect(&addr.to_string(), psk).await.unwrap();

    // Send invalid operation mode byte (0xFF)
    client.write_all(&[0xFF]).await.unwrap();

    let result = server_task.await.unwrap();
    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), 0xFF);
}

// ─── File download protocol tests ───────────────────────────────────────────

#[tokio::test]
async fn test_file_download_nonexistent_file() {
    let psk = "test_download_missing";

    let listener = NoiseListener::new("127.0.0.1:0", psk).await.unwrap();
    let addr = listener.local_addr().unwrap();

    // Create a temp file name that doesn't exist
    let server_task = tokio::spawn(async move {
        let mut layer = listener.accept().await.unwrap();

        // Read operation mode + file path
        let mut buffer = vec![0u8; 8192];
        let n = layer.read(&mut buffer).await.unwrap();

        assert_eq!(
            OperationMode::try_from(buffer[0]).unwrap(),
            OperationMode::GetFile
        );

        // Try to validate the path — this should work (relative path)
        let path_end = buffer[1..n].iter().position(|&b| b == 0).unwrap() + 1;
        let _file_path = std::str::from_utf8(&buffer[1..path_end]).unwrap();

        // Send zero size to indicate file not found
        layer.write_all(&0u64.to_be_bytes()).await.unwrap();
    });

    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    let mut client = NoiseLayer::connect(&addr.to_string(), psk).await.unwrap();

    let mut data = Vec::with_capacity(1 + 30 + 1);
    data.push(OperationMode::GetFile as u8);
    data.extend_from_slice(b"nonexistent_file_12345.txt");
    data.push(0);
    client.write_all(&data).await.unwrap();

    // Read file size — should be 0 (not found)
    let mut size_buf = [0u8; 8];
    client.read_exact(&mut size_buf).await.unwrap();
    let file_size = u64::from_be_bytes(size_buf);
    assert_eq!(file_size, 0);

    server_task.await.unwrap();
}

// ─── File upload protocol tests ─────────────────────────────────────────────

#[tokio::test]
async fn test_file_upload_round_trip() {
    let psk = "test_upload_round_trip";
    let test_content = b"Hello, upload test!";

    let temp_dir = std::env::temp_dir();
    let upload_path = temp_dir.join("tsh_upload_test.txt");

    let listener = NoiseListener::new("127.0.0.1:0", psk).await.unwrap();
    let addr = listener.local_addr().unwrap();

    let upload_path_clone = upload_path.clone();
    let server_task = tokio::spawn(async move {
        let mut layer = listener.accept().await.unwrap();

        // Read mode + file path
        let mut buffer = vec![0u8; 8192];
        let n = layer.read(&mut buffer).await.unwrap();

        assert_eq!(
            OperationMode::try_from(buffer[0]).unwrap(),
            OperationMode::PutFile
        );

        let path_end = buffer[1..n].iter().position(|&b| b == 0).unwrap() + 1;
        let file_path = std::str::from_utf8(&buffer[1..path_end]).unwrap();

        // Read file size
        let mut size_buf = [0u8; 8];
        layer.read_exact(&mut size_buf).await.unwrap();
        let file_size = u64::from_be_bytes(size_buf);

        // Read file data
        let mut file_data = vec![0u8; file_size as usize];
        layer.read_exact(&mut file_data).await.unwrap();

        // Write to disk
        tokio::fs::write(file_path, &file_data).await.unwrap();

        file_data
    });

    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    let mut client = NoiseLayer::connect(&addr.to_string(), psk).await.unwrap();

    // Send upload request
    let mut data = Vec::new();
    data.push(OperationMode::PutFile as u8);
    data.extend_from_slice(upload_path_clone.to_string_lossy().as_bytes());
    data.push(0);
    client.write_all(&data).await.unwrap();

    // Send file size + content
    let file_size = test_content.len() as u64;
    client.write_all(&file_size.to_be_bytes()).await.unwrap();
    client.write_all(test_content).await.unwrap();

    let received_data = server_task.await.unwrap();
    assert_eq!(received_data, test_content);

    // Verify file was written
    let written = tokio::fs::read(&upload_path).await.unwrap();
    assert_eq!(written, test_content);

    // Cleanup
    let _ = tokio::fs::remove_file(&upload_path).await;
}

// ─── Command execution tests ────────────────────────────────────────────────

#[tokio::test]
async fn test_command_result_protocol() {
    let psk = "test_cmd_result";

    let listener = NoiseListener::new("127.0.0.1:0", psk).await.unwrap();
    let addr = listener.local_addr().unwrap();

    let server_task = tokio::spawn(async move {
        let mut layer = listener.accept().await.unwrap();

        // Read command request
        let mut buffer = vec![0u8; 8192];
        let n = layer.read(&mut buffer).await.unwrap();

        assert_eq!(
            OperationMode::try_from(buffer[0]).unwrap(),
            OperationMode::RunCommand
        );

        let cmd_end = buffer[1..n].iter().position(|&b| b == 0).unwrap() + 1;
        let command = std::str::from_utf8(&buffer[1..cmd_end]).unwrap();
        assert_eq!(command, "echo server_test");

        // Simulate command output
        let stdout_data = b"server_test\n";
        let stderr_data = b"";

        // Send exit code
        layer.write_all(&[0u8]).await.unwrap();

        // Send stdout
        let stdout_len = stdout_data.len() as u32;
        layer.write_all(&stdout_len.to_be_bytes()).await.unwrap();
        layer.write_all(stdout_data).await.unwrap();

        // Send stderr
        let stderr_len = stderr_data.len() as u32;
        layer.write_all(&stderr_len.to_be_bytes()).await.unwrap();
    });

    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    let mut client = NoiseLayer::connect(&addr.to_string(), psk).await.unwrap();

    // Send command
    let mut data = Vec::new();
    data.push(OperationMode::RunCommand as u8);
    data.extend_from_slice(b"echo server_test");
    data.push(0);
    client.write_all(&data).await.unwrap();

    // Read exit code
    let mut exit_code_buf = [0u8; 1];
    client.read_exact(&mut exit_code_buf).await.unwrap();
    assert_eq!(exit_code_buf[0], 0);

    // Read stdout
    let mut stdout_len_buf = [0u8; 4];
    client.read_exact(&mut stdout_len_buf).await.unwrap();
    let stdout_len = u32::from_be_bytes(stdout_len_buf) as usize;
    assert_eq!(stdout_len, 12); // "server_test\n"

    let mut stdout_data = vec![0u8; stdout_len];
    client.read_exact(&mut stdout_data).await.unwrap();
    assert_eq!(String::from_utf8_lossy(&stdout_data).trim(), "server_test");

    // Read stderr
    let mut stderr_len_buf = [0u8; 4];
    client.read_exact(&mut stderr_len_buf).await.unwrap();
    let stderr_len = u32::from_be_bytes(stderr_len_buf) as usize;
    assert_eq!(stderr_len, 0);

    server_task.await.unwrap();
}
