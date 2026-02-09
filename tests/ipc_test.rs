use ansible_piloteer::ipc::{IpcServer, Message};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixStream;

#[tokio::test]
async fn test_ipc_unix_socket_handshake() {
    // Create a temporary path for the socket
    let socket_path = "test_ipc.sock";
    // Ensure cleanup
    let _ = tokio::fs::remove_file(socket_path).await;

    // Start Server
    let server_handle = tokio::spawn(async move {
        let server = IpcServer::new(socket_path, None)
            .await
            .expect("Failed to create server");
        let mut conn = server.accept().await.expect("Failed to accept connection");

        // Expect Handshake
        let msg = conn
            .receive()
            .await
            .expect("Failed to receive")
            .expect("Stream Closed");
        if let Message::Handshake { token } = msg {
            assert_eq!(token.as_deref(), Some("secret123"));
            // Send Proceed
            conn.send(&Message::Proceed)
                .await
                .expect("Failed to send Proceed");
        } else {
            panic!("Expected Handshake, got {:?}", msg);
        }
    });

    // Give server time to bind
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Start Client (Simulate Ansible Plugin)
    let mut stream = UnixStream::connect(socket_path)
        .await
        .expect("Failed to connect to socket");

    // Send Handshake
    let handshake = serde_json::json!({
        "Handshake": {
            "token": "secret123"
        }
    });
    let mut data = serde_json::to_string(&handshake).unwrap();
    data.push('\n');
    stream
        .write_all(data.as_bytes())
        .await
        .expect("Failed to write");

    // Wait for response
    let mut buf = vec![0u8; 1024];
    let n = stream.read(&mut buf).await.expect("Failed to read");
    let response = String::from_utf8_lossy(&buf[..n]);

    assert!(response.contains("Proceed"));

    // Cleanup
    server_handle.await.expect("Server task failed");
    let _ = tokio::fs::remove_file(socket_path).await;
}
