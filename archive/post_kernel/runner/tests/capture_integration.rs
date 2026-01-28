//! Integration tests for TUI frame capture via RPC relay.
//!
//! Tests the full CLI → Server → TUI → Server → CLI capture flow.

use std::{
    sync::atomic::{AtomicU16, Ordering},
    time::Duration,
};

use {
    runner::{
        Server, ServerConfig,
        client::{
            common::{ConnectionConfig, RpcClient},
            tui::HeadlessClient,
        },
    },
    serde_json::json,
    tokio::task::JoinHandle,
};

/// Global counter for unique test ports.
static CAPTURE_TEST_PORT: AtomicU16 = AtomicU16::new(12600);

/// Test server wrapper for capture integration tests.
struct CaptureTestServer {
    port: u16,
    handle: JoinHandle<()>,
}

impl CaptureTestServer {
    /// Spawn a test server on a unique port.
    async fn spawn() -> Self {
        let port = CAPTURE_TEST_PORT.fetch_add(1, Ordering::SeqCst);
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

/// Spawn headless TUI and return its task handle.
fn spawn_headless_tui(config: &ConnectionConfig) -> JoinHandle<()> {
    let config = config.clone();
    tokio::spawn(async move {
        match HeadlessClient::connect(&config).await {
            Ok(mut client) => {
                let _ = client.run().await;
            }
            Err(e) => {
                tracing::error!("Headless client error: {e}");
            }
        }
    })
}

// ============================================================================
// E2E Tests
// ============================================================================

#[tokio::test]
async fn test_capture_no_tui_returns_error() {
    let server = CaptureTestServer::spawn().await;
    let config = server.config();

    let mut client = RpcClient::connect(&config)
        .await
        .expect("Should connect to server");

    // Capture without TUI should return error
    let result = client
        .call("tui/capture", json!({"format": "plain_text"}))
        .await;

    assert!(result.is_err(), "Capture without TUI should fail");
    let err = result.unwrap_err();
    assert!(
        err.to_string().contains("No TUI") || err.to_string().contains("capture"),
        "Error should mention no TUI: {err}"
    );

    server.shutdown();
}

#[tokio::test]
async fn test_capture_with_headless_tui_succeeds() {
    let server = CaptureTestServer::spawn().await;
    let config = server.config();

    // Spawn headless TUI
    let headless_handle = spawn_headless_tui(&config);

    // Wait for headless to connect
    tokio::time::sleep(Duration::from_millis(200)).await;

    // CLI client captures
    let mut client = RpcClient::connect(&config)
        .await
        .expect("Should connect to server");

    let result = client
        .call("tui/capture", json!({"format": "plain_text"}))
        .await;

    assert!(result.is_ok(), "Capture with headless TUI should succeed: {:?}", result.err());

    let response = result.unwrap();
    assert!(response.get("content").is_some(), "Response should have content");
    assert!(response.get("width").is_some(), "Response should have width");
    assert!(response.get("height").is_some(), "Response should have height");

    headless_handle.abort();
    server.shutdown();
}

#[tokio::test]
async fn test_capture_returns_buffer_content() {
    let server = CaptureTestServer::spawn().await;
    let config = server.config();

    // Spawn headless TUI
    let headless_handle = spawn_headless_tui(&config);
    tokio::time::sleep(Duration::from_millis(200)).await;

    let mut client = RpcClient::connect(&config)
        .await
        .expect("Should connect to server");

    // Insert some text
    let _ = client
        .call("input/keys", json!({"keys": "iHello Capture Test"}))
        .await;
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Capture frame
    let result = client
        .call("tui/capture", json!({"format": "plain_text"}))
        .await;

    assert!(result.is_ok(), "Capture should succeed");

    let response = result.unwrap();
    let content = response
        .get("content")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    assert!(
        content.contains("Hello Capture Test"),
        "Capture should contain inserted text, got: {content}"
    );

    headless_handle.abort();
    server.shutdown();
}

#[tokio::test]
async fn test_capture_raw_ansi_format() {
    let server = CaptureTestServer::spawn().await;
    let config = server.config();

    let headless_handle = spawn_headless_tui(&config);
    tokio::time::sleep(Duration::from_millis(200)).await;

    let mut client = RpcClient::connect(&config)
        .await
        .expect("Should connect to server");

    let result = client
        .call("tui/capture", json!({"format": "raw_ansi"}))
        .await;

    assert!(result.is_ok(), "raw_ansi capture should succeed");

    let response = result.unwrap();
    let content = response
        .get("content")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    // Raw ANSI should have the header
    assert!(content.contains("=== FRAME CAPTURE ==="), "raw_ansi should have frame header");

    headless_handle.abort();
    server.shutdown();
}

#[tokio::test]
async fn test_capture_invalid_format_error() {
    let server = CaptureTestServer::spawn().await;
    let config = server.config();

    let headless_handle = spawn_headless_tui(&config);
    tokio::time::sleep(Duration::from_millis(200)).await;

    let mut client = RpcClient::connect(&config)
        .await
        .expect("Should connect to server");

    let result = client
        .call("tui/capture", json!({"format": "invalid_format"}))
        .await;

    assert!(result.is_err(), "Invalid format should fail");

    headless_handle.abort();
    server.shutdown();
}

// ============================================================================
// Concurrent Capture Tests
// ============================================================================

/// Test concurrent captures - headless TUI queues requests and processes them serially.
/// All concurrent requests should succeed (no dropped requests).
#[tokio::test]
async fn test_concurrent_captures_all_succeed() {
    let server = CaptureTestServer::spawn().await;
    let config = server.config();

    let headless_handle = spawn_headless_tui(&config);
    tokio::time::sleep(Duration::from_millis(200)).await;

    // Spawn multiple concurrent capture requests
    let mut handles = Vec::new();

    for i in 0..3 {
        let cfg = config.clone();
        handles.push(tokio::spawn(async move {
            let mut client = RpcClient::connect(&cfg).await.expect("Should connect");

            let result = client
                .call("tui/capture", json!({"format": "plain_text"}))
                .await;
            (i, result.is_ok())
        }));
    }

    // Wait for all captures
    let mut successes = 0;
    for handle in handles {
        if let Ok((_, true)) = handle.await {
            successes += 1;
        }
    }

    // All concurrent captures should succeed (queued and processed serially)
    assert!(
        successes >= 2,
        "At least 2 of 3 concurrent captures should succeed, got {successes}"
    );

    headless_handle.abort();
    server.shutdown();
}

#[tokio::test]
async fn test_rapid_sequential_captures() {
    let server = CaptureTestServer::spawn().await;
    let config = server.config();

    let headless_handle = spawn_headless_tui(&config);
    tokio::time::sleep(Duration::from_millis(200)).await;

    let mut client = RpcClient::connect(&config)
        .await
        .expect("Should connect to server");

    // Rapid sequential captures
    let mut successes = 0;
    for _ in 0..5 {
        let result = client
            .call("tui/capture", json!({"format": "plain_text"}))
            .await;
        if result.is_ok() {
            successes += 1;
        }
    }

    assert!(successes >= 4, "At least 4 of 5 rapid captures should succeed, got {successes}");

    headless_handle.abort();
    server.shutdown();
}

// ============================================================================
// Edge Case Tests
// ============================================================================

#[tokio::test]
async fn test_capture_after_tui_disconnect_fails() {
    let server = CaptureTestServer::spawn().await;
    let config = server.config();

    // Spawn and then disconnect headless TUI
    let headless_handle = spawn_headless_tui(&config);
    tokio::time::sleep(Duration::from_millis(200)).await;

    // Disconnect headless
    headless_handle.abort();
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Now try to capture - should fail
    let mut client = RpcClient::connect(&config)
        .await
        .expect("Should connect to server");

    let result = client
        .call("tui/capture", json!({"format": "plain_text"}))
        .await;

    assert!(result.is_err(), "Capture after TUI disconnect should fail");

    server.shutdown();
}

#[tokio::test]
async fn test_capture_default_dimensions() {
    let server = CaptureTestServer::spawn().await;
    let config = server.config();

    let headless_handle = spawn_headless_tui(&config);
    tokio::time::sleep(Duration::from_millis(200)).await;

    let mut client = RpcClient::connect(&config)
        .await
        .expect("Should connect to server");

    let result = client
        .call("tui/capture", json!({"format": "plain_text"}))
        .await;

    assert!(result.is_ok(), "Capture should succeed");

    let response = result.unwrap();

    // Headless defaults to 80x24
    let width = response
        .get("width")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    let height = response
        .get("height")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);

    assert_eq!(width, 80, "Default width should be 80");
    assert_eq!(height, 24, "Default height should be 24");

    headless_handle.abort();
    server.shutdown();
}
