use crate::constants::OperationMode;
use crate::helpers::NoiseLayerExt;
use crate::noise::{NoiseLayer, NoiseListener};
use tokio::process::Command;

#[tokio::test]
async fn test_command_execution_integration() {
    let psk = "test_cmd_integration";
    let test_command = if cfg!(windows) {
        "echo integration test"
    } else {
        "echo 'integration test'"
    };

    // Start listener
    let listener = NoiseListener::new("127.0.0.1:0", psk).await.unwrap();
    let addr = listener.local_addr().unwrap();

    // Spawn server task that mimics server behavior
    let server_task = tokio::spawn(async move {
        let mut layer = listener.accept().await.unwrap();

        // Read command request
        let mut buffer = vec![0u8; 1024];
        let n = layer.read(&mut buffer).await.unwrap();

        // Parse command
        let mode = OperationMode::from(buffer[0]);
        assert_eq!(mode, OperationMode::RunCommand);

        let cmd_end = buffer[1..n].iter().position(|&b| b == 0).unwrap() + 1;
        let command = String::from_utf8_lossy(&buffer[1..cmd_end]);

        // Execute command cross-platform
        let output = if cfg!(windows) {
            Command::new("cmd")
                .args(["/C", &command])
                .output()
                .await
                .unwrap()
        } else {
            Command::new("/bin/sh")
                .arg("-c")
                .arg(&*command)
                .output()
                .await
                .unwrap()
        };

        // Send response like server does
        let exit_code = if output.status.success() { 0u8 } else { 1u8 };
        layer.write_all(&[exit_code]).await.unwrap();

        let stdout_len = output.stdout.len() as u32;
        layer.write_all(&stdout_len.to_be_bytes()).await.unwrap();
        if stdout_len > 0 {
            layer.write_all(&output.stdout).await.unwrap();
        }

        let stderr_len = output.stderr.len() as u32;
        layer.write_all(&stderr_len.to_be_bytes()).await.unwrap();
        if stderr_len > 0 {
            layer.write_all(&output.stderr).await.unwrap();
        }

        String::from_utf8_lossy(&output.stdout).to_string()
    });

    // Give server time to start
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // Connect as client and send command
    let mut client = NoiseLayer::connect(&addr.to_string(), psk).await.unwrap();

    // Send command request like client does
    let mut data = Vec::new();
    data.push(OperationMode::RunCommand as u8);
    data.extend_from_slice(test_command.as_bytes());
    data.push(0);
    client.write_all(&data).await.unwrap();

    // Read response like client does
    let mut exit_code_buf = [0u8; 1];
    client.read_exact(&mut exit_code_buf).await.unwrap();
    let exit_code = exit_code_buf[0];

    let mut stdout_len_buf = [0u8; 4];
    client.read_exact(&mut stdout_len_buf).await.unwrap();
    let stdout_len = u32::from_be_bytes(stdout_len_buf) as usize;

    let mut stdout_data = vec![0u8; stdout_len];
    if stdout_len > 0 {
        client.read_exact(&mut stdout_data).await.unwrap();
    }

    let mut stderr_len_buf = [0u8; 4];
    client.read_exact(&mut stderr_len_buf).await.unwrap();
    let stderr_len = u32::from_be_bytes(stderr_len_buf) as usize;

    let mut stderr_data = vec![0u8; stderr_len];
    if stderr_len > 0 {
        client.read_exact(&mut stderr_data).await.unwrap();
    }

    // Wait for server task
    let server_output = server_task.await.unwrap();

    assert_eq!(exit_code, 0);
    assert_eq!(
        String::from_utf8_lossy(&stdout_data).trim(),
        "integration test"
    );
    assert_eq!(stderr_data.len(), 0);
    assert_eq!(server_output.trim(), "integration test");
}

#[tokio::test]
async fn test_file_operations_integration() {
    let psk = "test_file_integration";

    // Create test file in platform-appropriate temp directory
    let temp_dir = std::env::temp_dir();
    let test_file_path = temp_dir.join("tsh_test_integration.txt");
    let test_content = "This is integration test content";
    tokio::fs::write(&test_file_path, test_content)
        .await
        .unwrap();

    // Test file download
    {
        let listener = NoiseListener::new("127.0.0.1:0", psk).await.unwrap();
        let addr = listener.local_addr().unwrap();

        // Spawn server task for download
        let server_task = tokio::spawn(async move {
            let mut layer = listener.accept().await.unwrap();

            // Read download request
            let mut buffer = vec![0u8; 1024];
            let n = layer.read(&mut buffer).await.unwrap();

            // Parse request
            let mode = OperationMode::from(buffer[0]);
            assert_eq!(mode, OperationMode::GetFile);

            let path_end = buffer[1..n].iter().position(|&b| b == 0).unwrap() + 1;
            let file_path = String::from_utf8_lossy(&buffer[1..path_end]);

            // Read file and send back
            match tokio::fs::read(&*file_path).await {
                Ok(file_data) => {
                    let file_size = file_data.len() as u64;
                    layer.write_all(&file_size.to_be_bytes()).await.unwrap();
                    layer.write_all(&file_data).await.unwrap();
                    String::from_utf8_lossy(&file_data).to_string()
                }
                Err(_) => {
                    layer.write_all(&0u64.to_be_bytes()).await.unwrap();
                    String::new()
                }
            }
        });

        // Give server time to start
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        // Connect as client and request download
        let mut client = NoiseLayer::connect(&addr.to_string(), psk).await.unwrap();

        let mut data = Vec::new();
        data.push(OperationMode::GetFile as u8);
        data.extend_from_slice(test_file_path.to_string_lossy().as_bytes());
        data.push(0);
        client.write_all(&data).await.unwrap();

        // Read response
        let mut size_buf = [0u8; 8];
        client.read_exact(&mut size_buf).await.unwrap();
        let file_size = u64::from_be_bytes(size_buf);

        assert!(file_size > 0);

        let mut file_data = vec![0u8; file_size as usize];
        client.read_exact(&mut file_data).await.unwrap();

        let server_content = server_task.await.unwrap();
        let received_content = String::from_utf8_lossy(&file_data);

        assert_eq!(received_content, test_content);
        assert_eq!(server_content, test_content);
    }

    // Clean up
    let _ = tokio::fs::remove_file(&test_file_path).await;
}

#[tokio::test]
async fn test_multiple_connections() {
    let psk = "test_multi_connections";

    // Start listener
    let listener = NoiseListener::new("127.0.0.1:0", psk).await.unwrap();
    let addr = listener.local_addr().unwrap();

    // Spawn server task that handles multiple connections
    let server_task = tokio::spawn(async move {
        let mut connection_count = 0;

        for _ in 0..3 {
            let mut layer = listener.accept().await.unwrap();
            connection_count += 1;

            // Echo back any data received
            let mut buffer = vec![0u8; 1024];
            let n = layer.read(&mut buffer).await.unwrap();
            layer.write(&buffer[..n]).await.unwrap();
        }

        connection_count
    });

    // Give server time to start
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // Create multiple client connections
    let mut client_tasks = Vec::new();

    for i in 0..3 {
        let addr_str = addr.to_string();
        let psk_clone = psk.to_string();
        let test_data = format!("test data {i}");

        let client_task = tokio::spawn(async move {
            let mut client = NoiseLayer::connect(&addr_str, &psk_clone).await.unwrap();
            client.write(test_data.as_bytes()).await.unwrap();

            let mut buffer = vec![0u8; 1024];
            let n = client.read(&mut buffer).await.unwrap();
            buffer.truncate(n);

            String::from_utf8_lossy(&buffer).to_string()
        });

        client_tasks.push(client_task);

        // Small delay between connections
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
    }

    // Wait for all client tasks to complete
    let mut results = Vec::new();
    for task in client_tasks {
        results.push(task.await.unwrap());
    }

    // Wait for server task
    let connection_count = server_task.await.unwrap();

    assert_eq!(connection_count, 3);
    assert_eq!(results.len(), 3);

    for (i, result) in results.iter().enumerate() {
        assert_eq!(result, &format!("test data {i}"));
    }
}

#[tokio::test]
async fn test_error_handling() {
    let psk = "test_error_handling";

    // Test connection to non-existent server
    let result = NoiseLayer::connect("127.0.0.1:0", psk).await;
    assert!(result.is_err());

    // Test wrong PSK
    let listener = NoiseListener::new("127.0.0.1:0", psk).await.unwrap();
    let addr = listener.local_addr().unwrap();

    let server_task = tokio::spawn(async move { listener.accept().await });

    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    let wrong_psk_result = NoiseLayer::connect(&addr.to_string(), "wrong_psk").await;
    assert!(wrong_psk_result.is_err());

    // Cancel server task
    server_task.abort();
}
