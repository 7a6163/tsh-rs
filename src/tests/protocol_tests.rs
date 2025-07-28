use crate::constants::OperationMode;
use crate::error::TshResult;
use crate::helpers::NoiseLayerExt;
use crate::noise::{NoiseLayer, NoiseListener};

#[tokio::test]
async fn test_operation_mode_serialization() {
    assert_eq!(OperationMode::GetFile as u8, 1);
    assert_eq!(OperationMode::PutFile as u8, 2);
    assert_eq!(OperationMode::RunShell as u8, 3);
    assert_eq!(OperationMode::RunCommand as u8, 4);

    assert_eq!(OperationMode::from(1), OperationMode::GetFile);
    assert_eq!(OperationMode::from(2), OperationMode::PutFile);
    assert_eq!(OperationMode::from(3), OperationMode::RunShell);
    assert_eq!(OperationMode::from(4), OperationMode::RunCommand);
}

#[tokio::test]
async fn test_command_protocol() {
    let psk = "test_cmd_protocol";
    let command = "echo hello";

    // Start listener
    let listener = NoiseListener::new("127.0.0.1:0", psk).await.unwrap();
    let addr = listener.local_addr().unwrap();

    // Spawn server task that expects command protocol
    let server_task = tokio::spawn(async move {
        let mut layer = listener.accept().await.unwrap();

        // Read the complete message (mode + command + null terminator)
        let mut buffer = vec![0u8; 1024];
        let n = layer.read(&mut buffer).await.unwrap();

        // Parse the message
        let mode = OperationMode::from(buffer[0]);
        let cmd_end = buffer[1..n].iter().position(|&b| b == 0).unwrap() + 1;
        let received_command = String::from_utf8_lossy(&buffer[1..cmd_end]);

        (mode, received_command.to_string())
    });

    // Give server time to start
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // Connect as client and send command
    let mut client = NoiseLayer::connect(&addr.to_string(), psk).await.unwrap();

    // Send command in the same format as client
    let mut data = Vec::new();
    data.push(OperationMode::RunCommand as u8);
    data.extend_from_slice(command.as_bytes());
    data.push(0); // null terminator
    client.write_all(&data).await.unwrap();

    // Wait for server to parse the message
    let (received_mode, received_command) = server_task.await.unwrap();

    assert_eq!(received_mode, OperationMode::RunCommand);
    assert_eq!(received_command, command);
}

#[tokio::test]
async fn test_file_download_protocol() {
    let psk = "test_download_protocol";
    let file_path = "/tmp/test_file.txt";

    // Start listener
    let listener = NoiseListener::new("127.0.0.1:0", psk).await.unwrap();
    let addr = listener.local_addr().unwrap();

    // Spawn server task that expects file download protocol
    let server_task = tokio::spawn(async move {
        let mut layer = listener.accept().await.unwrap();

        // Read the complete message (mode + file path + null terminator)
        let mut buffer = vec![0u8; 1024];
        let n = layer.read(&mut buffer).await.unwrap();

        // Parse the message
        let mode = OperationMode::from(buffer[0]);
        let path_end = buffer[1..n].iter().position(|&b| b == 0).unwrap() + 1;
        let received_path = String::from_utf8_lossy(&buffer[1..path_end]);

        (mode, received_path.to_string())
    });

    // Give server time to start
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // Connect as client and send file download request
    let mut client = NoiseLayer::connect(&addr.to_string(), psk).await.unwrap();

    // Send file download request in the same format as client
    let mut data = Vec::new();
    data.push(OperationMode::GetFile as u8);
    data.extend_from_slice(file_path.as_bytes());
    data.push(0); // null terminator
    client.write_all(&data).await.unwrap();

    // Wait for server to parse the message
    let (received_mode, received_path) = server_task.await.unwrap();

    assert_eq!(received_mode, OperationMode::GetFile);
    assert_eq!(received_path, file_path);
}

#[tokio::test]
async fn test_file_upload_protocol() {
    let psk = "test_upload_protocol";
    let file_path = "/tmp/upload_test.txt";
    let file_size = 1234u64;

    // Start listener
    let listener = NoiseListener::new("127.0.0.1:0", psk).await.unwrap();
    let addr = listener.local_addr().unwrap();

    // Spawn server task that expects file upload protocol
    let server_task = tokio::spawn(async move {
        let mut layer = listener.accept().await.unwrap();

        // Read the complete message (mode + file path + null terminator)
        let mut buffer = vec![0u8; 1024];
        let n = layer.read(&mut buffer).await.unwrap();

        // Parse the message
        let mode = OperationMode::from(buffer[0]);
        let path_end = buffer[1..n].iter().position(|&b| b == 0).unwrap() + 1;
        let received_path = String::from_utf8_lossy(&buffer[1..path_end]);

        // Read file size (separate message)
        let mut size_buf = [0u8; 8];
        layer.read_exact(&mut size_buf).await.unwrap();
        let received_size = u64::from_be_bytes(size_buf);

        (mode, received_path.to_string(), received_size)
    });

    // Give server time to start
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // Connect as client and send file upload request
    let mut client = NoiseLayer::connect(&addr.to_string(), psk).await.unwrap();

    // Send file upload request in the same format as client
    let mut data = Vec::new();
    data.push(OperationMode::PutFile as u8);
    data.extend_from_slice(file_path.as_bytes());
    data.push(0); // null terminator
    client.write_all(&data).await.unwrap();

    // Send file size
    client.write_all(&file_size.to_be_bytes()).await.unwrap();

    // Wait for server to parse the message
    let (received_mode, received_path, received_size) = server_task.await.unwrap();

    assert_eq!(received_mode, OperationMode::PutFile);
    assert_eq!(received_path, file_path);
    assert_eq!(received_size, file_size);
}

#[tokio::test]
async fn test_shell_mode_protocol() {
    let psk = "test_shell_protocol";

    // Start listener
    let listener = NoiseListener::new("127.0.0.1:0", psk).await.unwrap();
    let addr = listener.local_addr().unwrap();

    // Spawn server task that expects shell mode protocol
    let server_task = tokio::spawn(async move {
        let mut layer = listener.accept().await.unwrap();

        // Read the operation mode
        let mut buffer = vec![0u8; 1024];
        let n = layer.read(&mut buffer).await.unwrap();

        // Parse the mode (should be just the mode byte for shell)
        let mode = OperationMode::from(buffer[0]);

        mode
    });

    // Give server time to start
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // Connect as client and send shell mode request
    let mut client = NoiseLayer::connect(&addr.to_string(), psk).await.unwrap();

    // Send shell mode request
    client
        .write_all(&[OperationMode::RunShell as u8])
        .await
        .unwrap();

    // Wait for server to parse the message
    let received_mode = server_task.await.unwrap();

    assert_eq!(received_mode, OperationMode::RunShell);
}

#[tokio::test]
async fn test_helpers_write_all() {
    let psk = "test_helpers";
    let test_data = b"Hello, helpers!";

    // Start listener
    let listener = NoiseListener::new("127.0.0.1:0", psk).await.unwrap();
    let addr = listener.local_addr().unwrap();

    // Spawn server task
    let server_task = tokio::spawn(async move {
        let mut layer = listener.accept().await.unwrap();

        // Read data using NoiseLayer's read method
        let mut buffer = vec![0u8; 1024];
        let n = layer.read(&mut buffer).await.unwrap();
        buffer.truncate(n);
        buffer
    });

    // Give server time to start
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // Connect as client and use write_all helper
    let mut client = NoiseLayer::connect(&addr.to_string(), psk).await.unwrap();
    client.write_all(test_data).await.unwrap();

    // Wait for server to receive data
    let received_data = server_task.await.unwrap();

    assert_eq!(received_data, test_data);
}

#[tokio::test]
async fn test_helpers_read_exact() {
    let psk = "test_read_exact";
    let test_data = b"Exact read test";

    // Start listener
    let listener = NoiseListener::new("127.0.0.1:0", psk).await.unwrap();
    let addr = listener.local_addr().unwrap();

    // Spawn server task
    let server_task = tokio::spawn(async move {
        let mut layer = listener.accept().await.unwrap();

        // Send test data
        layer.write(test_data).await.unwrap();
    });

    // Give server time to start
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // Connect as client and use read_exact helper
    let mut client = NoiseLayer::connect(&addr.to_string(), psk).await.unwrap();

    // Wait for server to send data
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // Read exact amount of data
    let mut buffer = vec![0u8; test_data.len()];
    client.read_exact(&mut buffer).await.unwrap();

    // Wait for server task to complete
    server_task.await.unwrap();

    assert_eq!(buffer, test_data);
}
