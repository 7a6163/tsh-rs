use crate::noise::{NoiseLayer, NoiseListener};
use crate::error::TshResult;
use tokio::net::TcpStream;

#[tokio::test]
async fn test_noise_key_generation() {
    let result = NoiseListener::new("127.0.0.1:0", "test_psk").await;
    assert!(result.is_ok());
    
    let listener = result.unwrap();
    let public_key = listener.public_key().unwrap();
    assert_eq!(public_key.len(), 32); // X25519 public key is 32 bytes
}

#[tokio::test]
async fn test_noise_handshake() {
    let psk = "test_handshake_psk";
    
    // Start listener
    let listener = NoiseListener::new("127.0.0.1:0", psk).await.unwrap();
    let addr = listener.local_addr().unwrap();
    
    // Spawn server task
    let server_task = tokio::spawn(async move {
        listener.accept().await
    });
    
    // Give server time to start
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    
    // Connect as client
    let client_result = NoiseLayer::connect(&addr.to_string(), psk).await;
    assert!(client_result.is_ok());
    
    // Wait for server to complete handshake
    let server_result = server_task.await.unwrap();
    assert!(server_result.is_ok());
}

#[tokio::test]
async fn test_noise_data_transmission() {
    let psk = "test_data_psk";
    let test_data = b"Hello, Noise Protocol!";
    
    // Start listener
    let listener = NoiseListener::new("127.0.0.1:0", psk).await.unwrap();
    let addr = listener.local_addr().unwrap();
    
    // Spawn server task
    let test_data_clone = test_data.to_vec();
    let server_task = tokio::spawn(async move {
        let mut layer = listener.accept().await.unwrap();
        
        // Read data from client
        let mut buffer = vec![0u8; 1024];
        let n = layer.read(&mut buffer).await.unwrap();
        buffer.truncate(n);
        
        // Echo back the data
        layer.write(&buffer).await.unwrap();
        
        buffer
    });
    
    // Give server time to start
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    
    // Connect as client and send data
    let mut client = NoiseLayer::connect(&addr.to_string(), psk).await.unwrap();
    client.write(test_data).await.unwrap();
    
    // Read echoed data
    let mut buffer = vec![0u8; 1024];
    let n = client.read(&mut buffer).await.unwrap();
    buffer.truncate(n);
    
    // Wait for server to complete
    let server_data = server_task.await.unwrap();
    
    assert_eq!(buffer, test_data);
    assert_eq!(server_data, test_data);
}

#[tokio::test]
async fn test_noise_wrong_psk() {
    let correct_psk = "correct_psk";
    let wrong_psk = "wrong_psk";
    
    // Start listener with correct PSK
    let listener = NoiseListener::new("127.0.0.1:0", correct_psk).await.unwrap();
    let addr = listener.local_addr().unwrap();
    
    // Spawn server task
    let server_task = tokio::spawn(async move {
        listener.accept().await
    });
    
    // Give server time to start
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    
    // Try to connect with wrong PSK
    let client_result = NoiseLayer::connect(&addr.to_string(), wrong_psk).await;
    
    // Should fail due to PSK mismatch
    assert!(client_result.is_err());
    
    // Cancel server task
    server_task.abort();
}

#[tokio::test]
async fn test_noise_large_message() {
    let psk = "test_large_psk";
    let large_data: Vec<u8> = (0..10000).map(|i| (i % 256) as u8).collect();
    
    // Start listener
    let listener = NoiseListener::new("127.0.0.1:0", psk).await.unwrap();
    let addr = listener.local_addr().unwrap();
    
    // Spawn server task
    let server_task = tokio::spawn(async move {
        let mut layer = listener.accept().await.unwrap();
        
        // Read data from client
        let mut buffer = vec![0u8; 20000];
        let n = layer.read(&mut buffer).await.unwrap();
        buffer.truncate(n);
        buffer
    });
    
    // Give server time to start
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    
    // Connect as client and send large data
    let mut client = NoiseLayer::connect(&addr.to_string(), psk).await.unwrap();
    client.write(&large_data).await.unwrap();
    
    // Wait for server to receive data
    let received_data = server_task.await.unwrap();
    
    assert_eq!(received_data, large_data);
}