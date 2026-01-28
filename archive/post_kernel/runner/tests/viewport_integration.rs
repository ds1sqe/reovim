//! Integration tests for per-client viewport architecture.
//!
//! These tests verify that multiple clients can have independent:
//! - Terminal dimensions (via `editor/resize`)
//! - Active buffers (via `editor/set_active_buffer`)
//! - Cursor positions (each client's active buffer cursor is independent)
//!
//! # Test Architecture
//!
//! Each test spawns a server on a unique port and connects multiple clients
//! to verify viewport independence.

use std::{
    sync::atomic::{AtomicU16, Ordering},
    time::Duration,
};

use {
    runner::{
        Server, ServerConfig,
        client::common::{ConnectionConfig, RpcClient},
    },
    serde_json::{Value, json},
    tokio::task::JoinHandle,
};

/// Global counter for unique test ports.
static TEST_PORT: AtomicU16 = AtomicU16::new(12600);

/// Test server wrapper for viewport integration tests.
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

/// Test that each client has independent terminal dimensions.
///
/// Two clients connect and resize to different dimensions.
/// Each client's `state/screen` should reflect their own dimensions.
#[tokio::test]
async fn test_different_terminal_sizes() {
    let server = TestServer::spawn().await;
    let config = server.config();

    // Connect two clients
    let mut client1 = RpcClient::connect(&config)
        .await
        .expect("Client 1 should connect");
    let mut client2 = RpcClient::connect(&config)
        .await
        .expect("Client 2 should connect");

    // Resize to different dimensions
    let resize1 = client1
        .call("editor/resize", json!({"width": 80, "height": 24}))
        .await;
    assert!(resize1.is_ok(), "Client 1 resize should succeed");

    let resize2 = client2
        .call("editor/resize", json!({"width": 200, "height": 50}))
        .await;
    assert!(resize2.is_ok(), "Client 2 resize should succeed");

    // Verify each client sees their own dimensions
    let screen1 = client1
        .call("state/screen", json!({}))
        .await
        .expect("Client 1 state/screen should succeed");
    let screen2 = client2
        .call("state/screen", json!({}))
        .await
        .expect("Client 2 state/screen should succeed");

    assert_eq!(
        screen1.get("width").and_then(Value::as_u64),
        Some(80),
        "Client 1 should have width 80"
    );
    assert_eq!(
        screen1.get("height").and_then(Value::as_u64),
        Some(24),
        "Client 1 should have height 24"
    );

    assert_eq!(
        screen2.get("width").and_then(Value::as_u64),
        Some(200),
        "Client 2 should have width 200"
    );
    assert_eq!(
        screen2.get("height").and_then(Value::as_u64),
        Some(50),
        "Client 2 should have height 50"
    );

    server.shutdown();
}

/// Test that resize of one client doesn't affect another.
///
/// This verifies the per-client viewport isolation.
#[tokio::test]
async fn test_resize_isolation() {
    let server = TestServer::spawn().await;
    let config = server.config();

    let mut client1 = RpcClient::connect(&config)
        .await
        .expect("Client 1 should connect");
    let mut client2 = RpcClient::connect(&config)
        .await
        .expect("Client 2 should connect");

    // Client 1 resizes first
    let _ = client1
        .call("editor/resize", json!({"width": 100, "height": 30}))
        .await;

    // Client 2 gets state/screen WITHOUT resizing
    // Should still have default dimensions (80x24)
    let screen2 = client2
        .call("state/screen", json!({}))
        .await
        .expect("Client 2 state/screen should succeed");

    assert_eq!(
        screen2.get("width").and_then(Value::as_u64),
        Some(80),
        "Client 2 should still have default width 80"
    );
    assert_eq!(
        screen2.get("height").and_then(Value::as_u64),
        Some(24),
        "Client 2 should still have default height 24"
    );

    server.shutdown();
}

/// Test that clients start with default viewport values (VT100: 80x24).
#[tokio::test]
async fn test_default_viewport_dimensions() {
    let server = TestServer::spawn().await;
    let config = server.config();

    let mut client = RpcClient::connect(&config)
        .await
        .expect("Client should connect");

    let screen = client
        .call("state/screen", json!({}))
        .await
        .expect("state/screen should succeed");

    // VT100 defaults
    assert_eq!(
        screen.get("width").and_then(Value::as_u64),
        Some(80),
        "Default width should be 80"
    );
    assert_eq!(
        screen.get("height").and_then(Value::as_u64),
        Some(24),
        "Default height should be 24"
    );

    server.shutdown();
}

/// Test that cursor returns a valid position.
///
/// With no active buffer, cursor defaults to (0, 0).
#[tokio::test]
async fn test_cursor_default_position() {
    let server = TestServer::spawn().await;
    let config = server.config();

    let mut client = RpcClient::connect(&config)
        .await
        .expect("Client should connect");

    let cursor = client
        .call("state/cursor", json!({}))
        .await
        .expect("state/cursor should succeed");

    // Default cursor position
    assert_eq!(cursor.get("line").and_then(Value::as_u64), Some(0), "Default line should be 0");
    assert_eq!(
        cursor.get("column").and_then(Value::as_u64),
        Some(0),
        "Default column should be 0"
    );

    server.shutdown();
}

/// Test that `editor/set_active_buffer` validates buffer existence.
///
/// Setting an invalid buffer ID should return an error.
#[tokio::test]
async fn test_set_active_buffer_invalid() {
    let server = TestServer::spawn().await;
    let config = server.config();

    let mut client = RpcClient::connect(&config)
        .await
        .expect("Client should connect");

    let result = client
        .call("editor/set_active_buffer", json!({"buffer_id": 9999}))
        .await;

    assert!(result.is_err(), "Setting invalid buffer ID should fail");

    server.shutdown();
}

/// Test that `editor/set_active_buffer` works with valid buffers.
///
/// Creates a buffer and switches to it, verifying the response.
#[tokio::test]
async fn test_set_active_buffer_success() {
    let server = TestServer::spawn().await;
    let config = server.config();

    let mut client = RpcClient::connect(&config)
        .await
        .expect("Client should connect");

    // First open a buffer (creates it in the session)
    let buffer_result = client
        .call("buffer/open_file", json!({"path": "/dev/null"}))
        .await
        .expect("buffer/open_file should succeed");

    let buffer_id = buffer_result
        .get("buffer_id")
        .and_then(Value::as_u64)
        .expect("Response should contain buffer_id");

    // Now set it as active
    let result = client
        .call("editor/set_active_buffer", json!({"buffer_id": buffer_id}))
        .await
        .expect("Setting valid buffer should succeed");

    assert!(
        result.get("ok").and_then(Value::as_bool).unwrap_or(false),
        "Response should contain ok: true"
    );

    // Verify previous_buffer_id is the same buffer (since buffer/open_file also activates it)
    assert_eq!(
        result.get("previous_buffer_id").and_then(Value::as_u64),
        Some(buffer_id),
        "Previous buffer should be the same buffer since open_file activates it"
    );

    server.shutdown();
}

/// Test that two clients can have independent active buffers.
///
/// Client 1 opens buffer A, Client 2 opens buffer B.
/// Each client should see their own active buffer.
#[tokio::test]
async fn test_two_clients_independent_active_buffers() {
    let server = TestServer::spawn().await;
    let config = server.config();

    let mut client1 = RpcClient::connect(&config)
        .await
        .expect("Client 1 should connect");
    let mut client2 = RpcClient::connect(&config)
        .await
        .expect("Client 2 should connect");

    // Client 1 opens buffer A
    let buf_open_1 = client1
        .call("buffer/open_file", json!({"path": "/dev/null"}))
        .await
        .expect("Client 1 buffer open should succeed");
    let first_buf_id = buf_open_1.get("buffer_id").and_then(Value::as_u64).unwrap();

    // Client 2 opens buffer B
    let buf_open_2 = client2
        .call("buffer/open_file", json!({"path": "/dev/null"}))
        .await
        .expect("Client 2 buffer open should succeed");
    let second_buf_id = buf_open_2.get("buffer_id").and_then(Value::as_u64).unwrap();

    // Client 1 sets active buffer to A
    let _ = client1
        .call("editor/set_active_buffer", json!({"buffer_id": first_buf_id}))
        .await
        .expect("Client 1 set_active_buffer should succeed");

    // Client 2 sets active buffer to B
    let _ = client2
        .call("editor/set_active_buffer", json!({"buffer_id": second_buf_id}))
        .await
        .expect("Client 2 set_active_buffer should succeed");

    // Verify each client has correct active buffer via state/screen
    let screen1 = client1
        .call("state/screen", json!({}))
        .await
        .expect("Client 1 state/screen should succeed");
    let screen2 = client2
        .call("state/screen", json!({}))
        .await
        .expect("Client 2 state/screen should succeed");

    assert_eq!(
        screen1.get("active_buffer_id").and_then(Value::as_u64),
        Some(first_buf_id),
        "Client 1 should have buffer A as active"
    );
    assert_eq!(
        screen2.get("active_buffer_id").and_then(Value::as_u64),
        Some(second_buf_id),
        "Client 2 should have buffer B as active"
    );

    server.shutdown();
}

/// Test that switching active buffer returns the previous buffer ID.
#[tokio::test]
async fn test_set_active_buffer_returns_previous() {
    let server = TestServer::spawn().await;
    let config = server.config();

    let mut client = RpcClient::connect(&config)
        .await
        .expect("Client should connect");

    // Open two buffers
    let first_open = client
        .call("buffer/open_file", json!({"path": "/dev/null"}))
        .await
        .expect("Buffer A open should succeed");
    let first_id = first_open.get("buffer_id").and_then(Value::as_u64).unwrap();

    let second_open = client
        .call("buffer/open_file", json!({"path": "/dev/null"}))
        .await
        .expect("Buffer B open should succeed");
    let second_id = second_open
        .get("buffer_id")
        .and_then(Value::as_u64)
        .unwrap();

    // Set buffer A as active first
    let _ = client
        .call("editor/set_active_buffer", json!({"buffer_id": first_id}))
        .await
        .expect("Set buffer A should succeed");

    // Now switch to buffer B - should return A as previous
    let switch_result = client
        .call("editor/set_active_buffer", json!({"buffer_id": second_id}))
        .await
        .expect("Set buffer B should succeed");

    assert_eq!(
        switch_result
            .get("previous_buffer_id")
            .and_then(Value::as_u64),
        Some(first_id),
        "Previous buffer should be buffer A"
    );

    server.shutdown();
}

/// Test that two clients can have different viewport sizes simultaneously.
///
/// Verifies that resize operations are truly isolated per-client.
#[tokio::test]
async fn test_three_clients_different_sizes() {
    let server = TestServer::spawn().await;
    let config = server.config();

    let mut client1 = RpcClient::connect(&config).await.expect("C1 connect");
    let mut client2 = RpcClient::connect(&config).await.expect("C2 connect");
    let mut client3 = RpcClient::connect(&config).await.expect("C3 connect");

    // Resize all three to different sizes
    client1
        .call("editor/resize", json!({"width": 80, "height": 24}))
        .await
        .unwrap();
    client2
        .call("editor/resize", json!({"width": 120, "height": 40}))
        .await
        .unwrap();
    client3
        .call("editor/resize", json!({"width": 200, "height": 60}))
        .await
        .unwrap();

    // Verify each has their own size
    let s1 = client1.call("state/screen", json!({})).await.unwrap();
    let s2 = client2.call("state/screen", json!({})).await.unwrap();
    let s3 = client3.call("state/screen", json!({})).await.unwrap();

    assert_eq!(s1.get("width").and_then(Value::as_u64), Some(80));
    assert_eq!(s1.get("height").and_then(Value::as_u64), Some(24));

    assert_eq!(s2.get("width").and_then(Value::as_u64), Some(120));
    assert_eq!(s2.get("height").and_then(Value::as_u64), Some(40));

    assert_eq!(s3.get("width").and_then(Value::as_u64), Some(200));
    assert_eq!(s3.get("height").and_then(Value::as_u64), Some(60));

    server.shutdown();
}
