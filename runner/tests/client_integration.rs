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
    assert!(servers.len() <= 10, "At most 10 ports scanned");
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

#[cfg(test)]
mod common {
    use super::*;

    #[test]
    fn test_connection_config_default() {
        let config = ConnectionConfig::tcp_default();
        match config {
            ConnectionConfig::Tcp { host, port } => {
                assert_eq!(host, "127.0.0.1");
                assert_eq!(port, 12521);
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
