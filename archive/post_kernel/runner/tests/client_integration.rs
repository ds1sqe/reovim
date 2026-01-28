//! Integration tests for TUI and CLI clients.
//!
//! These tests spawn a server and verify client functionality.

use std::{
    sync::atomic::{AtomicU16, Ordering},
    time::Duration,
};

use {
    runner::{
        Server, ServerConfig,
        client::common::{ConnectionConfig, RpcClient, discovery},
    },
    serde_json::json,
    tokio::task::JoinHandle,
};

/// Global counter for unique test ports.
static TEST_PORT: AtomicU16 = AtomicU16::new(12540);

/// Test server wrapper for integration tests.
struct TestServer {
    port: u16,
    handle: JoinHandle<()>,
}

impl TestServer {
    /// Spawn a test server on a unique port.
    async fn spawn() -> Self {
        // Get a unique port for this test
        let port = TEST_PORT.fetch_add(1, Ordering::SeqCst);
        let config = ServerConfig::tcp(port);
        let server = Server::new(config);

        let handle = tokio::spawn(async move {
            let _ = server.run().await;
        });

        // Wait for server to start
        tokio::time::sleep(Duration::from_millis(300)).await;

        Self { port, handle }
    }

    /// Get connection config for this server.
    fn config(&self) -> ConnectionConfig {
        ConnectionConfig::tcp("127.0.0.1", self.port)
    }

    /// Shutdown the server.
    fn shutdown(self) {
        self.handle.abort();
    }
}

#[tokio::test]
async fn test_cli_list_servers() {
    // Discovery should work even without a running server
    let servers = discovery::list_servers();
    // We can't assert exact count since other servers might be running
    // Just verify the function doesn't panic
    // Note: Since #446, we also scan /proc for reovim processes on any port,
    // so there's no upper limit on the number of servers discovered.
    assert!(
        servers.iter().all(|s| s.port > 0),
        "All discovered servers should have valid ports"
    );
}

/// Test that `server/kill` RPC actually terminates the server (#446).
///
/// This is an end-to-end test that verifies the shutdown signal propagates
/// from the RPC handler through the watch channel to the accept loop.
#[tokio::test]
async fn test_server_kill_terminates_server() {
    let server = TestServer::spawn().await;
    let port = server.port;
    let config = server.config();

    // Connect and send kill command
    let mut client = RpcClient::connect(&config)
        .await
        .expect("Should connect to server");

    let result = client.call("server/kill", json!({})).await;
    assert!(result.is_ok(), "server/kill should succeed");

    // Give the server time to process shutdown
    tokio::time::sleep(Duration::from_millis(200)).await;

    // Verify server is no longer accepting connections
    let reconnect_result = RpcClient::connect(&ConnectionConfig::tcp("127.0.0.1", port)).await;
    assert!(
        reconnect_result.is_err(),
        "Server should no longer accept connections after kill"
    );

    // Clean up the handle (server already shut down, but abort anyway)
    server.handle.abort();
}

#[tokio::test]
async fn test_rpc_client_connect_and_call() {
    let server = TestServer::spawn().await;
    let config = server.config();

    let mut client = RpcClient::connect(&config)
        .await
        .expect("Should connect to server");

    // Call state/mode - should return current mode
    let result = client.call("state/mode", json!({})).await;
    assert!(result.is_ok(), "state/mode should succeed");

    let mode = result.unwrap();
    // Should have an edit_mode field
    assert!(
        mode.get("edit_mode").is_some() || mode.get("display").is_some(),
        "Mode response should have edit_mode or display"
    );

    server.shutdown();
}

#[tokio::test]
async fn test_rpc_keys_accepted() {
    let server = TestServer::spawn().await;
    let config = server.config();

    let mut client = RpcClient::connect(&config)
        .await
        .expect("Should connect to server");

    // Send keys and verify the server accepts them
    let result = client.call("input/keys", json!({ "keys": "j" })).await;
    assert!(result.is_ok(), "input/keys should accept key sequence");

    // Verify mode query works
    let mode = client
        .call("state/mode", json!({}))
        .await
        .expect("state/mode should succeed");

    // Should have some mode information
    assert!(
        mode.get("edit_mode").is_some() || mode.get("display").is_some(),
        "Mode response should have edit_mode or display"
    );

    server.shutdown();
}

#[tokio::test]
async fn test_rpc_error_handling() {
    let server = TestServer::spawn().await;
    let config = server.config();

    let mut client = RpcClient::connect(&config)
        .await
        .expect("Should connect to server");

    // Call non-existent method
    let result = client.call("nonexistent/method", json!({})).await;

    assert!(result.is_err(), "Invalid method should return error");
    let err = result.unwrap_err();
    assert!(
        err.to_string().contains("not found") || err.to_string().contains("Unknown method"),
        "Error should indicate method not found"
    );

    server.shutdown();
}

#[tokio::test]
async fn test_rpc_buffer_list() {
    let server = TestServer::spawn().await;
    let config = server.config();

    let mut client = RpcClient::connect(&config)
        .await
        .expect("Should connect to server");

    let result = client
        .call("buffer/list", json!({}))
        .await
        .expect("buffer/list should succeed");

    // Should have a buffers array (may be empty if no buffers created yet)
    let buffers = result.get("buffers").and_then(|v| v.as_array());
    assert!(buffers.is_some(), "Should have buffers array");
    // Buffer list may be empty initially - just verify the array exists

    server.shutdown();
}

#[tokio::test]
async fn test_rpc_cursor_position() {
    let server = TestServer::spawn().await;
    let config = server.config();

    let mut client = RpcClient::connect(&config)
        .await
        .expect("Should connect to server");

    let result = client
        .call("state/cursor", json!({}))
        .await
        .expect("state/cursor should succeed");

    // Should have line and column
    assert!(result.get("line").is_some(), "Should have line");
    assert!(result.get("column").is_some(), "Should have column");

    server.shutdown();
}

#[tokio::test]
async fn test_rpc_client_into_split() {
    let server = TestServer::spawn().await;
    let config = server.config();

    // Connect and get initial state before split
    let mut client = RpcClient::connect(&config)
        .await
        .expect("Should connect to server");

    // Get initial mode to verify connection works
    let mode = client
        .call("state/mode", json!({}))
        .await
        .expect("Initial state/mode should succeed");
    assert!(mode.get("display").is_some(), "Should have mode info");

    // Split the client
    let (mut reader, mut writer) = client.into_split();

    // Send a request via the writer
    let request_id = writer
        .send_request("state/cursor", json!({}))
        .await
        .expect("Should send request");
    assert!(request_id > 0, "Request ID should be positive");

    // Read the response via the reader
    let line = reader.read_line().await.expect("Should read response");
    let response: serde_json::Value = serde_json::from_str(&line).expect("Should parse JSON");

    // Verify we got a response with cursor info
    assert!(
        response.get("result").is_some() || response.get("error").is_some(),
        "Should have result or error"
    );

    server.shutdown();
}

#[tokio::test]
async fn test_connection_reader_writer_independence() {
    use {
        runner::client::common::ConnectionConfig,
        tokio::{
            io::{AsyncBufReadExt, AsyncWriteExt},
            net::TcpListener,
        },
    };

    // Create a simple echo server for testing
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let server_handle = tokio::spawn(async move {
        let (socket, _) = listener.accept().await.unwrap();
        let (read_half, mut write_half) = socket.into_split();
        let mut reader = tokio::io::BufReader::new(read_half);
        let mut line = String::new();

        // Echo back what we receive
        while let Ok(n) = reader.read_line(&mut line).await {
            if n == 0 {
                break;
            }
            write_half.write_all(line.as_bytes()).await.unwrap();
            line.clear();
        }
    });

    // Connect and split
    let config = ConnectionConfig::tcp("127.0.0.1", addr.port());
    let connection = runner::client::common::Connection::connect(&config)
        .await
        .expect("Should connect");

    let (mut reader, mut writer) = connection.split();

    // Write some data
    writer
        .write_line("test message")
        .await
        .expect("Should write");

    // Read the echo
    let response = reader.read_line().await.expect("Should read");
    assert_eq!(response, "test message");

    server_handle.abort();
}

/// Test that TUI can connect and receive initial state.
#[tokio::test]
async fn test_tui_connect_gets_initial_state() {
    let server = TestServer::spawn().await;
    let config = server.config();

    // Connect using RpcClient (same path TuiApp uses)
    let mut client = RpcClient::connect(&config).await.expect("Should connect");

    // Get initial mode
    let mode = client
        .call("state/mode", json!({}))
        .await
        .expect("Should get mode");
    assert!(mode.get("display").is_some(), "Mode should have display field");

    // Get initial cursor
    let cursor = client
        .call("state/cursor", json!({}))
        .await
        .expect("Should get cursor");
    assert!(cursor.get("line").is_some(), "Cursor should have line");
    assert!(cursor.get("column").is_some(), "Cursor should have column");

    // Split for concurrent operation (same as TuiApp)
    let (_reader, mut writer) = client.into_split();

    // Verify writer can send requests
    let id = writer
        .send_request("state/mode", json!({}))
        .await
        .expect("Should send request");
    assert!(id > 0, "Request ID should be positive");

    server.shutdown();
}

/// Test notification listener reads messages correctly.
#[tokio::test]
async fn test_notification_listener_reads_messages() {
    use {
        reovim_protocol::v1::{RpcNotification, RpcResponse},
        runner::client::common::ServerMessage,
        tokio::sync::mpsc,
    };

    let server = TestServer::spawn().await;
    let config = server.config();

    let client = RpcClient::connect(&config).await.expect("Should connect");

    let (reader, mut writer) = client.into_split();

    // Create channel for messages
    let (tx, mut rx) = mpsc::channel(16);

    // Spawn listener (simplified version of TuiApp's listener)
    let listener = tokio::spawn(async move {
        let mut reader = reader;
        loop {
            let Ok(line) = reader.read_line().await else {
                break;
            };

            if let Ok(notification) = serde_json::from_str::<RpcNotification>(&line) {
                let _ = tx.send(ServerMessage::Notification(notification)).await;
            } else if let Ok(response) = serde_json::from_str::<RpcResponse>(&line) {
                let _ = tx.send(ServerMessage::Response(response)).await;
            }
        }
    });

    // Send a request and verify we get a response through the channel
    writer
        .send_request("state/mode", json!({}))
        .await
        .expect("Should send");

    // Wait for response with timeout
    let msg = tokio::time::timeout(Duration::from_secs(2), rx.recv())
        .await
        .expect("Should receive within timeout")
        .expect("Channel should not be closed");
    assert!(matches!(msg, ServerMessage::Response(_)), "Should receive a response");

    listener.abort();
    server.shutdown();
}

#[cfg(test)]
mod common {
    use super::*;

    #[test]
    fn test_connection_config_default() {
        let config = ConnectionConfig::tcp_default();
        match config {
            ConnectionConfig::Tcp { host, port } => {
                assert_eq!(host, "127.0.0.1");
                assert_eq!(port, 12522);
            }
            #[cfg(unix)]
            _ => panic!("Expected TCP config"),
        }
    }

    #[test]
    fn test_connection_config_parse() {
        let config = ConnectionConfig::parse_tcp("192.168.1.1:9000").unwrap();
        match config {
            ConnectionConfig::Tcp { host, port } => {
                assert_eq!(host, "192.168.1.1");
                assert_eq!(port, 9000);
            }
            #[cfg(unix)]
            _ => panic!("Expected TCP config"),
        }
    }

    #[test]
    fn test_server_info_builder() {
        let info = discovery::ServerInfo::new(12521).with_pid(1234);
        assert_eq!(info.port, 12521);
        assert_eq!(info.pid, Some(1234));
    }
}
