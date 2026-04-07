use crate::client;
use crate::constants::OperationMode;
use crate::helpers::NoiseLayerExt;
use crate::noise::{NoiseLayer, NoiseListener};

// ─── handle_connect_back_mode ────────────────────────────────────────────────

#[tokio::test]
async fn test_client_connect_back_command() {
    let psk = "test_client_cb_cmd";

    // In connect-back mode, the client listens and the server connects to it.
    // We simulate the server side: connect to the client, send a RunCommand mode byte,
    // then act as a command server.

    // Find a free port
    let tmp_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = tmp_listener.local_addr().unwrap().port();
    drop(tmp_listener);

    // Spawn client in connect-back mode with a command action
    let client_task = tokio::spawn(async move {
        client::handle_connect_back_mode(port, vec!["echo cb_test"], psk).await
    });

    // Give client time to start listening
    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

    // Simulate server connecting to client
    let mut server = NoiseLayer::connect(&format!("127.0.0.1:{port}"), psk)
        .await
        .unwrap();

    // Server sends RunCommand mode (the client reads this in connect_back_mode)
    server
        .write_all(&[OperationMode::RunCommand as u8])
        .await
        .unwrap();

    // Client reads mode and since it has actions, calls execute_action.
    // execute_action sends: mode byte + command + null terminator
    let mut buffer = vec![0u8; 8192];
    let n = server.read(&mut buffer).await.unwrap();

    assert_eq!(
        OperationMode::try_from(buffer[0]).unwrap(),
        OperationMode::RunCommand
    );

    let cmd_end = buffer[1..n].iter().position(|&b| b == 0).unwrap() + 1;
    let command = std::str::from_utf8(&buffer[1..cmd_end]).unwrap();
    assert_eq!(command, "echo cb_test");

    // Send response back to client
    server.write_all(&[0u8]).await.unwrap(); // exit code 0
    let output = b"cb_test\n";
    server
        .write_all(&(output.len() as u32).to_be_bytes())
        .await
        .unwrap();
    server.write_all(output).await.unwrap();
    server.write_all(&0u32.to_be_bytes()).await.unwrap(); // no stderr

    // Client should complete
    let result = tokio::time::timeout(tokio::time::Duration::from_secs(3), client_task)
        .await
        .unwrap()
        .unwrap();
    assert!(result.is_ok(), "Connect-back should succeed: {:?}", result.err());
}

// ─── handle_direct_connection: address formatting ───────────────────────────

#[tokio::test]
async fn test_client_direct_connection_host_port_format() {
    let psk = "test_client_hp_format";

    let listener = NoiseListener::new("127.0.0.1:0", psk).await.unwrap();
    let addr = listener.local_addr().unwrap();

    // Mock server
    let server_task = tokio::spawn(async move {
        let mut layer = listener.accept().await.unwrap();
        let mut buffer = vec![0u8; 8192];
        let _n = layer.read(&mut buffer).await.unwrap();

        // Respond to command
        layer.write_all(&[0u8]).await.unwrap();
        let output = b"result\n";
        layer
            .write_all(&(output.len() as u32).to_be_bytes())
            .await
            .unwrap();
        layer.write_all(output).await.unwrap();
        layer.write_all(&0u32.to_be_bytes()).await.unwrap();
    });

    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // Test with host:port (no separate port)
    let result = client::handle_direct_connection(
        &format!("127.0.0.1:{}", addr.port()),
        9999, // should be ignored when host contains ':'
        vec!["echo result"],
        psk,
    )
    .await;
    assert!(result.is_ok());

    let _ = server_task.await;
}

#[tokio::test]
async fn test_client_direct_connection_separate_port() {
    let psk = "test_client_sep_port";

    let listener = NoiseListener::new("127.0.0.1:0", psk).await.unwrap();
    let addr = listener.local_addr().unwrap();

    let server_task = tokio::spawn(async move {
        let mut layer = listener.accept().await.unwrap();
        let mut buffer = vec![0u8; 8192];
        let _n = layer.read(&mut buffer).await.unwrap();

        layer.write_all(&[0u8]).await.unwrap();
        layer.write_all(&0u32.to_be_bytes()).await.unwrap();
        layer.write_all(&0u32.to_be_bytes()).await.unwrap();
    });

    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // Test with separate host and port
    let result = client::handle_direct_connection(
        "127.0.0.1",
        addr.port(),
        vec!["echo test"],
        psk,
    )
    .await;
    assert!(result.is_ok());

    let _ = server_task.await;
}

// ─── execute_action: command execution ──────────────────────────────────────

#[tokio::test]
async fn test_client_execute_command() {
    let psk = "test_client_exec_cmd";

    let listener = NoiseListener::new("127.0.0.1:0", psk).await.unwrap();
    let addr = listener.local_addr().unwrap();

    // Mock server: read command, send back response
    let server_task = tokio::spawn(async move {
        let mut layer = listener.accept().await.unwrap();

        let mut buffer = vec![0u8; 8192];
        let n = layer.read(&mut buffer).await.unwrap();

        assert_eq!(
            OperationMode::try_from(buffer[0]).unwrap(),
            OperationMode::RunCommand
        );

        let cmd_end = buffer[1..n].iter().position(|&b| b == 0).unwrap() + 1;
        let command = std::str::from_utf8(&buffer[1..cmd_end]).unwrap();
        assert_eq!(command, "echo client_test");

        // Simulate successful response
        layer.write_all(&[0u8]).await.unwrap(); // exit code 0
        let stdout = b"client_test\n";
        layer
            .write_all(&(stdout.len() as u32).to_be_bytes())
            .await
            .unwrap();
        layer.write_all(stdout).await.unwrap();
        layer.write_all(&0u32.to_be_bytes()).await.unwrap(); // no stderr
    });

    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    let mut layer = NoiseLayer::connect(&addr.to_string(), psk).await.unwrap();
    let result = client::execute_action(&mut layer, vec!["echo client_test"]).await;
    assert!(result.is_ok());

    let _ = server_task.await;
}

// ─── execute_action: get file ───────────────────────────────────────────────

#[tokio::test]
async fn test_client_download_file() {
    let psk = "test_client_download";
    let file_content = b"downloaded content here";

    let listener = NoiseListener::new("127.0.0.1:0", psk).await.unwrap();
    let addr = listener.local_addr().unwrap();

    // Mock server: respond with file data
    let server_task = tokio::spawn(async move {
        let mut layer = listener.accept().await.unwrap();

        let mut buffer = vec![0u8; 8192];
        let _n = layer.read(&mut buffer).await.unwrap();

        assert_eq!(
            OperationMode::try_from(buffer[0]).unwrap(),
            OperationMode::GetFile
        );

        // Send file size + data
        let file_size = file_content.len() as u64;
        layer.write_all(&file_size.to_be_bytes()).await.unwrap();
        layer.write_all(file_content).await.unwrap();
    });

    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // Create temp directory for download
    let temp_dir = std::env::temp_dir();
    let download_dir = temp_dir.join("tsh_client_dl_test");
    tokio::fs::create_dir_all(&download_dir).await.unwrap();

    let mut layer = NoiseLayer::connect(&addr.to_string(), psk).await.unwrap();
    let action = format!("get:remote_file.txt:{}", download_dir.to_string_lossy());
    let result = client::execute_action(&mut layer, vec![&action]).await;
    assert!(result.is_ok(), "Download failed: {:?}", result.err());

    // Verify downloaded file
    let downloaded = tokio::fs::read(download_dir.join("remote_file.txt")).await;
    assert!(downloaded.is_ok(), "Downloaded file should exist");
    assert_eq!(downloaded.unwrap(), file_content);

    // Cleanup
    let _ = tokio::fs::remove_dir_all(&download_dir).await;
    let _ = server_task.await;
}

// ─── execute_action: put file ───────────────────────────────────────────────

#[tokio::test]
async fn test_client_upload_file() {
    let psk = "test_client_upload";
    let upload_content = b"content to upload";

    // Create local file to upload
    let temp_dir = std::env::temp_dir();
    let local_file = temp_dir.join("tsh_client_upload_src.txt");
    tokio::fs::write(&local_file, upload_content).await.unwrap();

    let listener = NoiseListener::new("127.0.0.1:0", psk).await.unwrap();
    let addr = listener.local_addr().unwrap();

    // Mock server: read upload request and data
    let server_task = tokio::spawn(async move {
        let mut layer = listener.accept().await.unwrap();

        let mut buffer = vec![0u8; 8192];
        let n = layer.read(&mut buffer).await.unwrap();

        assert_eq!(
            OperationMode::try_from(buffer[0]).unwrap(),
            OperationMode::PutFile
        );

        let path_end = buffer[1..n].iter().position(|&b| b == 0).unwrap() + 1;
        let remote_path = std::str::from_utf8(&buffer[1..path_end]).unwrap();
        assert!(
            remote_path.contains("tsh_client_upload_src.txt"),
            "Path should contain filename, got: {remote_path}"
        );

        // Read file size
        let mut size_buf = [0u8; 8];
        layer.read_exact(&mut size_buf).await.unwrap();
        let file_size = u64::from_be_bytes(size_buf);
        assert_eq!(file_size, upload_content.len() as u64);

        // Read file data
        let mut file_data = vec![0u8; file_size as usize];
        layer.read_exact(&mut file_data).await.unwrap();
        assert_eq!(file_data, upload_content);
    });

    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    let mut layer = NoiseLayer::connect(&addr.to_string(), psk).await.unwrap();
    let action = format!(
        "put:{}:/remote/dir",
        local_file.to_string_lossy()
    );
    let result = client::execute_action(&mut layer, vec![&action]).await;
    assert!(result.is_ok(), "Upload failed: {:?}", result.err());

    // Cleanup
    let _ = tokio::fs::remove_file(&local_file).await;
    let _ = server_task.await;
}

// ─── execute_action: cmd: prefix ────────────────────────────────────────────

#[tokio::test]
async fn test_client_cmd_prefix_action() {
    let psk = "test_client_cmd_prefix";

    let listener = NoiseListener::new("127.0.0.1:0", psk).await.unwrap();
    let addr = listener.local_addr().unwrap();

    let server_task = tokio::spawn(async move {
        let mut layer = listener.accept().await.unwrap();

        let mut buffer = vec![0u8; 8192];
        let n = layer.read(&mut buffer).await.unwrap();

        assert_eq!(
            OperationMode::try_from(buffer[0]).unwrap(),
            OperationMode::RunCommand
        );

        let cmd_end = buffer[1..n].iter().position(|&b| b == 0).unwrap() + 1;
        let command = std::str::from_utf8(&buffer[1..cmd_end]).unwrap();
        assert_eq!(command, "ls -la");

        // Send response
        layer.write_all(&[0u8]).await.unwrap();
        let output = b"file1.txt\n";
        layer
            .write_all(&(output.len() as u32).to_be_bytes())
            .await
            .unwrap();
        layer.write_all(output).await.unwrap();
        layer.write_all(&0u32.to_be_bytes()).await.unwrap();
    });

    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    let mut layer = NoiseLayer::connect(&addr.to_string(), psk).await.unwrap();
    let result = client::execute_action(&mut layer, vec!["cmd:ls -la"]).await;
    assert!(result.is_ok());

    let _ = server_task.await;
}

// ─── execute_action: cmd with colons ────────────────────────────────────────

#[tokio::test]
async fn test_client_cmd_with_colons() {
    let psk = "test_client_cmd_colon";

    let listener = NoiseListener::new("127.0.0.1:0", psk).await.unwrap();
    let addr = listener.local_addr().unwrap();

    let server_task = tokio::spawn(async move {
        let mut layer = listener.accept().await.unwrap();

        let mut buffer = vec![0u8; 8192];
        let n = layer.read(&mut buffer).await.unwrap();

        let cmd_end = buffer[1..n].iter().position(|&b| b == 0).unwrap() + 1;
        let command = std::str::from_utf8(&buffer[1..cmd_end]).unwrap();
        // "cmd:echo:hello:world" should become "echo:hello:world"
        assert_eq!(command, "echo:hello:world");

        layer.write_all(&[0u8]).await.unwrap();
        layer.write_all(&0u32.to_be_bytes()).await.unwrap();
        layer.write_all(&0u32.to_be_bytes()).await.unwrap();
    });

    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    let mut layer = NoiseLayer::connect(&addr.to_string(), psk).await.unwrap();
    let result = client::execute_action(&mut layer, vec!["cmd:echo:hello:world"]).await;
    assert!(result.is_ok());

    let _ = server_task.await;
}

// ─── execute_action: file not found ─────────────────────────────────────────

#[tokio::test]
async fn test_client_download_file_not_found() {
    let psk = "test_client_dl_404";

    let listener = NoiseListener::new("127.0.0.1:0", psk).await.unwrap();
    let addr = listener.local_addr().unwrap();

    let server_task = tokio::spawn(async move {
        let mut layer = listener.accept().await.unwrap();

        let mut buffer = vec![0u8; 8192];
        let _n = layer.read(&mut buffer).await.unwrap();

        // Send file size = 0 (not found)
        layer.write_all(&0u64.to_be_bytes()).await.unwrap();
    });

    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    let temp_dir = std::env::temp_dir();
    let mut layer = NoiseLayer::connect(&addr.to_string(), psk).await.unwrap();
    let action = format!("get:nonexistent.txt:{}", temp_dir.to_string_lossy());
    let result = client::execute_action(&mut layer, vec![&action]).await;

    // Should fail because file size is 0
    assert!(result.is_err());

    let _ = server_task.await;
}

// ─── address formatting ─────────────────────────────────────────────────────

#[tokio::test]
async fn test_client_address_with_port() {
    let psk = "test_client_addr";

    let listener = NoiseListener::new("127.0.0.1:0", psk).await.unwrap();
    let addr = listener.local_addr().unwrap();

    let server_task = tokio::spawn(async move {
        let mut layer = listener.accept().await.unwrap();
        let mut buffer = vec![0u8; 8192];
        let _n = layer.read(&mut buffer).await.unwrap();

        // Respond to command
        layer.write_all(&[0u8]).await.unwrap();
        let output = b"ok\n";
        layer
            .write_all(&(output.len() as u32).to_be_bytes())
            .await
            .unwrap();
        layer.write_all(output).await.unwrap();
        layer.write_all(&0u32.to_be_bytes()).await.unwrap();
    });

    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // Test handle_direct_connection with "host:port" format
    let result = client::handle_direct_connection(
        &addr.to_string(),
        addr.port(),
        vec!["echo ok"],
        psk,
    )
    .await;
    assert!(result.is_ok());

    let _ = server_task.await;
}
