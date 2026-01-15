//! Integration tests for debug RPC endpoints.
//!
//! These tests spawn a server and verify all 11 debug endpoints.

use std::{
    sync::atomic::{AtomicU16, Ordering},
    time::Duration,
};

use {
    runner::{
        Server, ServerConfig,
        client::common::{ConnectionConfig, RpcClient},
    },
    serde_json::json,
    tokio::task::JoinHandle,
};

/// Global counter for unique test ports.
/// Start at 12560 to avoid conflict with `client_integration` tests.
static TEST_PORT: AtomicU16 = AtomicU16::new(12560);

/// Test server wrapper for debug integration tests.
struct TestServer {
    port: u16,
    handle: JoinHandle<()>,
}

impl TestServer {
    /// Spawn a test server on a unique port.
    async fn spawn() -> Self {
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

// ============================================================================
// Phase 1: Foundation
// ============================================================================

#[tokio::test]
async fn test_debug_version() {
    let server = TestServer::spawn().await;
    let config = server.config();

    let mut client = RpcClient::connect(&config)
        .await
        .expect("Should connect to server");

    let result = client
        .call("debug/version", json!({}))
        .await
        .expect("debug/version should succeed");

    // Verify required fields
    assert!(result.get("version").is_some(), "Should have version field");
    assert!(result.get("rust_version").is_some(), "Should have rust_version field");
    // build_date and git_hash are optional (set at build time)

    server.shutdown();
}

#[tokio::test]
async fn test_debug_uptime() {
    let server = TestServer::spawn().await;
    let config = server.config();

    let mut client = RpcClient::connect(&config)
        .await
        .expect("Should connect to server");

    let result = client
        .call("debug/uptime", json!({}))
        .await
        .expect("debug/uptime should succeed");

    // Verify required fields
    let uptime_seconds = result
        .get("uptime_seconds")
        .and_then(serde_json::Value::as_f64);
    assert!(uptime_seconds.is_some(), "Should have uptime_seconds");
    assert!(uptime_seconds.unwrap() >= 0.0, "Uptime should be non-negative");

    assert!(result.get("uptime_human").is_some(), "Should have uptime_human field");
    assert!(result.get("start_time").is_some(), "Should have start_time field");

    server.shutdown();
}

// ============================================================================
// Phase 2: Inspection
// ============================================================================

#[tokio::test]
async fn test_debug_kernel_state() {
    let server = TestServer::spawn().await;
    let config = server.config();

    let mut client = RpcClient::connect(&config)
        .await
        .expect("Should connect to server");

    let result = client
        .call("debug/kernel_state", json!({}))
        .await
        .expect("debug/kernel_state should succeed");

    // Verify kernel state fields
    assert!(result.get("buffer_count").is_some(), "Should have buffer_count");
    assert!(result.get("event_handlers").is_some(), "Should have event_handlers");
    assert!(result.get("event_queue_len").is_some(), "Should have event_queue_len");

    server.shutdown();
}

#[tokio::test]
async fn test_debug_registers() {
    let server = TestServer::spawn().await;
    let config = server.config();

    let mut client = RpcClient::connect(&config)
        .await
        .expect("Should connect to server");

    let result = client
        .call("debug/registers", json!({}))
        .await
        .expect("debug/registers should succeed");

    // Verify register structure
    assert!(result.get("unnamed").is_some(), "Should have unnamed register");
    assert!(result.get("named").is_some(), "Should have named registers");

    // Named should be an array
    let named = result.get("named").and_then(|v| v.as_array());
    assert!(named.is_some(), "Named should be an array");

    server.shutdown();
}

#[tokio::test]
async fn test_debug_marks() {
    let server = TestServer::spawn().await;
    let config = server.config();

    let mut client = RpcClient::connect(&config)
        .await
        .expect("Should connect to server");

    let result = client
        .call("debug/marks", json!({}))
        .await
        .expect("debug/marks should succeed");

    // Verify marks structure
    assert!(result.get("local").is_some(), "Should have local marks array");
    assert!(result.get("global").is_some(), "Should have global marks array");
    assert!(result.get("special").is_some(), "Should have special marks array");

    server.shutdown();
}

#[tokio::test]
async fn test_debug_mode_stack() {
    let server = TestServer::spawn().await;
    let config = server.config();

    let mut client = RpcClient::connect(&config)
        .await
        .expect("Should connect to server");

    let result = client
        .call("debug/mode_stack", json!({}))
        .await
        .expect("debug/mode_stack should succeed");

    // Verify mode stack structure
    assert!(result.get("current").is_some(), "Should have current mode");
    assert!(result.get("stack").is_some(), "Should have stack array");
    assert!(result.get("depth").is_some(), "Should have depth");

    let depth = result.get("depth").and_then(serde_json::Value::as_u64);
    assert!(depth.is_some(), "Depth should be a number");
    assert!(depth.unwrap() >= 1, "Depth should be at least 1");

    server.shutdown();
}

// ============================================================================
// Phase 3: Metrics
// ============================================================================

#[tokio::test]
async fn test_debug_metrics() {
    let server = TestServer::spawn().await;
    let config = server.config();

    let mut client = RpcClient::connect(&config)
        .await
        .expect("Should connect to server");

    let result = client
        .call("debug/metrics", json!({}))
        .await
        .expect("debug/metrics should succeed");

    // Verify metrics fields
    assert!(result.get("uptime_seconds").is_some(), "Should have uptime_seconds");
    assert!(result.get("total_requests").is_some(), "Should have total_requests");
    assert!(result.get("counters").is_some(), "Should have counters array");

    server.shutdown();
}

#[tokio::test]
async fn test_debug_handlers() {
    let server = TestServer::spawn().await;
    let config = server.config();

    let mut client = RpcClient::connect(&config)
        .await
        .expect("Should connect to server");

    let result = client
        .call("debug/handlers", json!({}))
        .await
        .expect("debug/handlers should succeed");

    // Verify handlers array (may be empty initially)
    let handlers = result.get("handlers").and_then(|v| v.as_array());
    assert!(handlers.is_some(), "Should have handlers array");

    // If we have handler stats, verify their structure
    if let Some(first) = handlers.unwrap().first() {
        assert!(first.get("method").is_some(), "Handler should have method");
        assert!(first.get("call_count").is_some(), "Handler should have call_count");
    }

    server.shutdown();
}

// ============================================================================
// Phase 4: Log Access
// ============================================================================

#[tokio::test]
async fn test_debug_log_level() {
    let server = TestServer::spawn().await;
    let config = server.config();

    let mut client = RpcClient::connect(&config)
        .await
        .expect("Should connect to server");

    let result = client
        .call("debug/log_level", json!({}))
        .await
        .expect("debug/log_level should succeed");

    // Verify level field
    let level = result.get("level").and_then(|v| v.as_str());
    assert!(level.is_some(), "Should have level string");

    // Level should be one of the standard levels
    let valid_levels = ["ERROR", "WARN", "INFO", "DEBUG", "TRACE", "OFF"];
    assert!(
        valid_levels
            .iter()
            .any(|l| level.unwrap().to_uppercase().contains(l)),
        "Level should be a valid log level"
    );

    server.shutdown();
}

#[tokio::test]
async fn test_debug_log_tail() {
    let server = TestServer::spawn().await;
    let config = server.config();

    let mut client = RpcClient::connect(&config)
        .await
        .expect("Should connect to server");

    // Make a few RPC calls to generate some log activity
    let _ = client.call("debug/version", json!({})).await;
    let _ = client.call("debug/uptime", json!({})).await;

    let result = client
        .call("debug/log_tail", json!({ "count": 10 }))
        .await
        .expect("debug/log_tail should succeed");

    // Verify entries array
    assert!(result.get("entries").is_some(), "Should have entries array");
    assert!(result.get("overflow_count").is_some(), "Should have overflow_count");

    let entries = result.get("entries").and_then(|v| v.as_array());
    assert!(entries.is_some(), "Entries should be an array");

    // Check entry structure if we have any
    if let Some(first) = entries.unwrap().first() {
        assert!(first.get("timestamp").is_some(), "Entry should have timestamp");
        assert!(first.get("level").is_some(), "Entry should have level");
        assert!(first.get("target").is_some(), "Entry should have target");
        assert!(first.get("message").is_some(), "Entry should have message");
    }

    server.shutdown();
}

// ============================================================================
// Phase 5: Visual Debug
// ============================================================================

#[tokio::test]
async fn test_debug_visual_snapshot() {
    let server = TestServer::spawn().await;
    let config = server.config();

    let mut client = RpcClient::connect(&config)
        .await
        .expect("Should connect to server");

    let result = client
        .call("debug/visual_snapshot", json!({}))
        .await
        .expect("debug/visual_snapshot should succeed");

    // Verify all sections are present
    assert!(result.get("schema_version").is_some(), "Should have schema_version");
    assert!(result.get("timestamp").is_some(), "Should have timestamp");
    assert!(result.get("server").is_some(), "Should have server section");
    assert!(result.get("editor").is_some(), "Should have editor section");
    assert!(result.get("buffers").is_some(), "Should have buffers section");
    assert!(result.get("ui").is_some(), "Should have ui section");
    assert!(result.get("vim").is_some(), "Should have vim section");
    assert!(result.get("metrics").is_some(), "Should have metrics section");

    server.shutdown();
}

// ============================================================================
// Enhanced Tests: Value Verification
// ============================================================================

#[tokio::test]
async fn test_debug_version_value_format() {
    let server = TestServer::spawn().await;
    let config = server.config();

    let mut client = RpcClient::connect(&config)
        .await
        .expect("Should connect to server");

    let result = client
        .call("debug/version", json!({}))
        .await
        .expect("debug/version should succeed");

    // Verify version format (semver-like)
    let version = result.get("version").and_then(|v| v.as_str()).unwrap();
    assert!(version.contains('.'), "Version should be semver format: {version}");

    // Verify rust_version is not empty
    let rust_version = result.get("rust_version").and_then(|v| v.as_str()).unwrap();
    assert!(!rust_version.is_empty(), "rust_version should not be empty");

    server.shutdown();
}

#[tokio::test]
async fn test_debug_uptime_increases() {
    let server = TestServer::spawn().await;
    let config = server.config();

    let mut client = RpcClient::connect(&config)
        .await
        .expect("Should connect to server");

    // First call
    let result1 = client
        .call("debug/uptime", json!({}))
        .await
        .expect("First uptime call should succeed");

    let uptime1 = result1
        .get("uptime_seconds")
        .and_then(serde_json::Value::as_f64)
        .unwrap();

    // Small delay
    tokio::time::sleep(Duration::from_millis(50)).await;

    // Second call
    let result2 = client
        .call("debug/uptime", json!({}))
        .await
        .expect("Second uptime call should succeed");

    let uptime2 = result2
        .get("uptime_seconds")
        .and_then(serde_json::Value::as_f64)
        .unwrap();

    // Uptime should increase
    assert!(uptime2 >= uptime1, "Uptime should increase: {uptime1} -> {uptime2}");

    // Verify start_time is ISO format
    let start_time = result1.get("start_time").and_then(|v| v.as_str()).unwrap();
    assert!(
        start_time.contains('T') || start_time.contains('-'),
        "start_time should be ISO format: {start_time}"
    );

    server.shutdown();
}

#[tokio::test]
async fn test_debug_kernel_state_values() {
    let server = TestServer::spawn().await;
    let config = server.config();

    let mut client = RpcClient::connect(&config)
        .await
        .expect("Should connect to server");

    let result = client
        .call("debug/kernel_state", json!({}))
        .await
        .expect("debug/kernel_state should succeed");

    // buffer_count should be a non-negative number
    let buffer_count = result
        .get("buffer_count")
        .and_then(serde_json::Value::as_u64);
    assert!(buffer_count.is_some(), "buffer_count should be a number");

    // event_handlers should be a non-negative number
    let event_handlers = result
        .get("event_handlers")
        .and_then(serde_json::Value::as_u64);
    assert!(event_handlers.is_some(), "event_handlers should be a number");

    // buffer_ids should be an array
    let buffer_ids = result.get("buffer_ids").and_then(|v| v.as_array());
    assert!(buffer_ids.is_some(), "buffer_ids should be an array");

    server.shutdown();
}

#[tokio::test]
async fn test_debug_registers_unnamed_structure() {
    let server = TestServer::spawn().await;
    let config = server.config();

    let mut client = RpcClient::connect(&config)
        .await
        .expect("Should connect to server");

    let result = client
        .call("debug/registers", json!({}))
        .await
        .expect("debug/registers should succeed");

    // Verify unnamed register has expected fields
    let unnamed = result.get("unnamed").unwrap();
    assert!(unnamed.get("name").is_some(), "unnamed should have name");
    assert!(unnamed.get("content").is_some(), "unnamed should have content");
    assert!(unnamed.get("content_length").is_some(), "unnamed should have content_length");
    assert!(unnamed.get("yank_type").is_some(), "unnamed should have yank_type");

    server.shutdown();
}

#[tokio::test]
async fn test_debug_visual_snapshot_nested_values() {
    let server = TestServer::spawn().await;
    let config = server.config();

    let mut client = RpcClient::connect(&config)
        .await
        .expect("Should connect to server");

    let result = client
        .call("debug/visual_snapshot", json!({}))
        .await
        .expect("debug/visual_snapshot should succeed");

    // Verify schema_version format
    let schema_version = result
        .get("schema_version")
        .and_then(|v| v.as_str())
        .unwrap();
    assert!(
        schema_version.contains('.'),
        "schema_version should be semver: {schema_version}"
    );

    // Verify server section has expected fields
    let server_section = result.get("server").unwrap();
    assert!(server_section.get("version").is_some(), "server should have version");
    assert!(
        server_section.get("uptime_seconds").is_some(),
        "server should have uptime_seconds"
    );
    assert!(server_section.get("session_id").is_some(), "server should have session_id");

    // Verify editor section has expected fields
    let editor_section = result.get("editor").unwrap();
    assert!(editor_section.get("mode").is_some(), "editor should have mode");
    assert!(editor_section.get("cursor").is_some(), "editor should have cursor");

    // Verify buffers section has count
    let buffers_section = result.get("buffers").unwrap();
    assert!(buffers_section.get("count").is_some(), "buffers should have count");

    server.shutdown();
}

// ============================================================================
// Enhanced Tests: Edge Cases
// ============================================================================

#[tokio::test]
async fn test_debug_log_tail_count_zero() {
    let server = TestServer::spawn().await;
    let config = server.config();

    let mut client = RpcClient::connect(&config)
        .await
        .expect("Should connect to server");

    // Request zero entries
    let result = client
        .call("debug/log_tail", json!({ "count": 0 }))
        .await
        .expect("log_tail with count=0 should succeed");

    let entries = result.get("entries").and_then(|v| v.as_array()).unwrap();
    assert!(entries.is_empty(), "count=0 should return empty entries");

    server.shutdown();
}

#[tokio::test]
async fn test_debug_log_tail_large_count() {
    let server = TestServer::spawn().await;
    let config = server.config();

    let mut client = RpcClient::connect(&config)
        .await
        .expect("Should connect to server");

    // Request more entries than likely exist
    let result = client
        .call("debug/log_tail", json!({ "count": 10000 }))
        .await
        .expect("log_tail with large count should succeed");

    // Should return whatever entries exist, not error
    let entries = result.get("entries").and_then(|v| v.as_array());
    assert!(entries.is_some(), "Should return entries array even for large count");

    server.shutdown();
}

#[tokio::test]
async fn test_debug_log_tail_default_count() {
    let server = TestServer::spawn().await;
    let config = server.config();

    let mut client = RpcClient::connect(&config)
        .await
        .expect("Should connect to server");

    // Call without count parameter - should use default
    let result = client
        .call("debug/log_tail", json!({}))
        .await
        .expect("log_tail without count should succeed");

    assert!(result.get("entries").is_some(), "Should have entries with default count");
    assert!(result.get("overflow_count").is_some(), "Should have overflow_count");

    server.shutdown();
}

#[tokio::test]
async fn test_debug_metrics_counters_structure() {
    let server = TestServer::spawn().await;
    let config = server.config();

    let mut client = RpcClient::connect(&config)
        .await
        .expect("Should connect to server");

    let result = client
        .call("debug/metrics", json!({}))
        .await
        .expect("debug/metrics should succeed");

    // Verify total_requests is a number (may be 0 if metrics not wired up)
    let total_requests = result
        .get("total_requests")
        .and_then(serde_json::Value::as_u64);
    assert!(total_requests.is_some(), "total_requests should be a number");

    // Verify uptime_seconds is positive
    let uptime = result
        .get("uptime_seconds")
        .and_then(serde_json::Value::as_f64)
        .unwrap();
    assert!(uptime >= 0.0, "uptime_seconds should be non-negative");

    // Verify counters array structure
    let counters = result.get("counters").and_then(|v| v.as_array()).unwrap();
    for counter in counters {
        assert!(counter.get("name").is_some(), "counter should have name");
        assert!(counter.get("value").is_some(), "counter should have value");
    }

    server.shutdown();
}

#[tokio::test]
async fn test_debug_mode_stack_current_not_empty() {
    let server = TestServer::spawn().await;
    let config = server.config();

    let mut client = RpcClient::connect(&config)
        .await
        .expect("Should connect to server");

    let result = client
        .call("debug/mode_stack", json!({}))
        .await
        .expect("debug/mode_stack should succeed");

    // Current mode should not be empty
    let current = result.get("current").and_then(|v| v.as_str()).unwrap();
    assert!(!current.is_empty(), "current mode should not be empty");

    // Stack should be an array with at least one entry
    let stack = result.get("stack").and_then(|v| v.as_array()).unwrap();
    assert!(!stack.is_empty(), "stack should have at least one entry");

    server.shutdown();
}

// ============================================================================
// Enhanced Tests: Error Paths
// ============================================================================

#[tokio::test]
async fn test_debug_unknown_method() {
    let server = TestServer::spawn().await;
    let config = server.config();

    let mut client = RpcClient::connect(&config)
        .await
        .expect("Should connect to server");

    // Call non-existent debug method
    let result = client.call("debug/nonexistent", json!({})).await;

    assert!(result.is_err(), "Unknown debug method should return error");

    server.shutdown();
}

#[tokio::test]
async fn test_debug_handlers_structure() {
    let server = TestServer::spawn().await;
    let config = server.config();

    let mut client = RpcClient::connect(&config)
        .await
        .expect("Should connect to server");

    let result = client
        .call("debug/handlers", json!({}))
        .await
        .expect("debug/handlers should succeed");

    // Verify handlers is an array (may be empty if metrics not wired up)
    let handlers = result.get("handlers").and_then(|v| v.as_array());
    assert!(handlers.is_some(), "handlers should be an array");

    // If handlers have entries, verify their structure
    if let Some(handlers) = handlers {
        for handler in handlers {
            assert!(handler.get("method").is_some(), "handler should have method");
            assert!(handler.get("call_count").is_some(), "handler should have call_count");
            assert!(handler.get("total_micros").is_some(), "handler should have total_micros");
            assert!(handler.get("avg_micros").is_some(), "handler should have avg_micros");
        }
    }

    server.shutdown();
}
