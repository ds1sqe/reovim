use super::*;

use std::path::PathBuf;

use reovim_driver_lsp::{LspServerConfig, client::Client, jsonrpc};

fn make_uri(path: &str) -> Uri {
    path.parse().expect("test URI should parse")
}

/// Create a test handle with default server state.
fn make_test_handle(tx: mpsc::Sender<LspRequest>, active: bool) -> LspSaturatorHandle {
    LspSaturatorHandle {
        tx,
        cache: Arc::new(DiagnosticCache::new()),
        active: Arc::new(AtomicBool::new(active)),
        capabilities: Arc::new(CapabilityStore::new(lsp_types::ServerCapabilities::default())),
        root_path: PathBuf::from("/tmp/test"),
        language_id: "rust".to_string(),
        server_info: None,
    }
}

/// Create a test `CapabilityStore` for request handler tests.
fn make_test_capability_store() -> CapabilityStore {
    CapabilityStore::new(lsp_types::ServerCapabilities::default())
}

/// Create a test client using `cat` to keep stdio pipes alive.
fn make_test_client() -> Arc<Client> {
    let config = LspServerConfig {
        command: "cat".to_string(),
        args: vec![],
        root_path: PathBuf::from("/tmp"),
        workspace_folders: vec![],
    };
    let (client, _reader, _stderr) = Client::spawn(config).expect("cat should be available");
    Arc::new(client)
}

#[test]
fn test_handle_send_request_success() {
    let (tx, _rx) = mpsc::channel::<LspRequest>(10);
    let handle = make_test_handle(tx, true);
    assert!(handle.send_request(LspRequest::Shutdown));
}

#[test]
fn test_handle_send_request_channel_full() {
    let (tx, _rx) = mpsc::channel::<LspRequest>(1);
    // Fill the channel
    tx.try_send(LspRequest::Shutdown).unwrap();
    let handle = make_test_handle(tx, true);
    assert!(!handle.send_request(LspRequest::Shutdown));
}

#[test]
fn test_handle_send_when_channel_closed() {
    let (tx, rx) = mpsc::channel::<LspRequest>(1);
    drop(rx);
    let handle = make_test_handle(tx, false);
    assert!(!handle.send_request(LspRequest::Shutdown));
}

#[test]
fn test_handle_diagnostics_returns_cache() {
    let handle = make_test_handle(mpsc::channel::<LspRequest>(1).0, false);
    assert!(handle.diagnostics().is_empty());
}

#[test]
fn test_handle_is_active_initially_false() {
    let handle = make_test_handle(mpsc::channel::<LspRequest>(1).0, false);
    assert!(!handle.is_active());
}

#[test]
fn test_handle_is_active_when_set() {
    let handle = make_test_handle(mpsc::channel::<LspRequest>(1).0, true);
    assert!(handle.is_active());
}

#[test]
fn test_handle_debug() {
    let handle = make_test_handle(mpsc::channel::<LspRequest>(1).0, true);
    let debug = format!("{handle:?}");
    assert!(debug.contains("LspSaturatorHandle"));
    assert!(debug.contains("active: true"));
}

#[test]
fn test_handle_clone() {
    let handle = make_test_handle(mpsc::channel::<LspRequest>(1).0, true);
    let cloned = handle.clone();
    assert_eq!(cloned.is_active(), handle.is_active());
}

#[test]
fn test_handle_diagnostics() {
    let uri = make_uri("file:///test.rs");
    let mut handle = make_test_handle(mpsc::channel::<LspRequest>(1).0, true);
    handle.cache = Arc::new(DiagnosticCache::new());
    handle.cache.store(&uri, Some(1), vec![]);
    assert!(handle.diagnostics().has(&uri));
}

#[test]
fn test_handle_capabilities_when_active() {
    let handle = make_test_handle(mpsc::channel::<LspRequest>(1).0, true);
    assert!(handle.capabilities().is_some());
}

#[test]
fn test_handle_capabilities_when_inactive() {
    let handle = make_test_handle(mpsc::channel::<LspRequest>(1).0, false);
    assert!(handle.capabilities().is_none());
}

#[test]
fn test_handle_root_path() {
    let handle = make_test_handle(mpsc::channel::<LspRequest>(1).0, true);
    assert_eq!(handle.root_path(), std::path::Path::new("/tmp/test"));
}

#[test]
fn test_handle_language_id() {
    let handle = make_test_handle(mpsc::channel::<LspRequest>(1).0, true);
    assert_eq!(handle.language_id(), "rust");
}

#[test]
fn test_handle_server_info_absent() {
    let handle = make_test_handle(mpsc::channel::<LspRequest>(1).0, true);
    assert!(handle.server_info().is_none());
}

#[test]
fn test_handle_server_info_present() {
    let mut handle = make_test_handle(mpsc::channel::<LspRequest>(1).0, true);
    handle.server_info = Some(lsp_types::ServerInfo {
        name: "test-server".to_string(),
        version: Some("1.0.0".to_string()),
    });
    let info = handle.server_info().unwrap();
    assert_eq!(info.name, "test-server");
}

#[test]
fn test_handle_diagnostics_stores_push_diagnostics() {
    let uri = make_uri("file:///src/main.rs");
    let cache = Arc::new(DiagnosticCache::new());

    let params = PublishDiagnosticsParams {
        uri: uri.clone(),
        diagnostics: vec![],
        version: Some(1),
    };

    LspSaturator::handle_diagnostics(&cache, params);
    assert!(cache.has(&uri));
}

// --- Tests calling actual functions (coverage) ---

#[tokio::test]
async fn test_handle_server_message_with_response() {
    // Enable debug-level tracing so debug!() field expressions are evaluated
    let _ = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .with_test_writer()
        .try_init();

    let client = make_test_client();
    let cache = Arc::new(DiagnosticCache::new());

    let response = jsonrpc::Response::success(jsonrpc::Id::Number(99), serde_json::Value::Null);
    let message = jsonrpc::Message::Response(response);

    // No pending request for ID 99 — logs warning, no panic
    LspSaturator::handle_server_message(&client, &cache, &make_test_capability_store(), message)
        .await;
}

#[tokio::test]
async fn test_handle_server_message_with_diagnostics_notification() {
    let client = make_test_client();
    let cache = Arc::new(DiagnosticCache::new());

    let uri: Uri = "file:///test.rs".parse().unwrap();
    let params = PublishDiagnosticsParams {
        uri: uri.clone(),
        diagnostics: vec![],
        version: Some(1),
    };

    let notification = jsonrpc::Notification::new(
        "textDocument/publishDiagnostics",
        Some(serde_json::to_value(params).unwrap()),
    );
    let message = jsonrpc::Message::Notification(notification);

    LspSaturator::handle_server_message(&client, &cache, &make_test_capability_store(), message)
        .await;
    assert!(cache.has(&uri));
}

#[tokio::test]
async fn test_handle_server_message_diagnostics_no_params() {
    let client = make_test_client();
    let cache = Arc::new(DiagnosticCache::new());

    let notification = jsonrpc::Notification::new("textDocument/publishDiagnostics", None);
    let message = jsonrpc::Message::Notification(notification);

    LspSaturator::handle_server_message(&client, &cache, &make_test_capability_store(), message)
        .await;
    assert!(cache.is_empty());
}

#[tokio::test]
async fn test_handle_server_message_diagnostics_invalid_params() {
    let client = make_test_client();
    let cache = Arc::new(DiagnosticCache::new());

    let notification = jsonrpc::Notification::new(
        "textDocument/publishDiagnostics",
        Some(serde_json::json!({"invalid": "data"})),
    );
    let message = jsonrpc::Message::Notification(notification);

    LspSaturator::handle_server_message(&client, &cache, &make_test_capability_store(), message)
        .await;
    assert!(cache.is_empty());
}

#[tokio::test]
async fn test_handle_server_message_unhandled_notification() {
    let client = make_test_client();
    let cache = Arc::new(DiagnosticCache::new());

    let notification = jsonrpc::Notification::new(
        "window/logMessage",
        Some(serde_json::json!({"type": 3, "message": "test"})),
    );
    let message = jsonrpc::Message::Notification(notification);

    LspSaturator::handle_server_message(&client, &cache, &make_test_capability_store(), message)
        .await;
}

#[tokio::test]
async fn test_handle_server_message_with_request() {
    let client = make_test_client();
    let cache = Arc::new(DiagnosticCache::new());

    let request = jsonrpc::Request::new(1_i64, "client/registerCapability", None);
    let message = jsonrpc::Message::Request(request);

    LspSaturator::handle_server_message(&client, &cache, &make_test_capability_store(), message)
        .await;
}

#[tokio::test]
async fn test_handle_server_request_register_capability_direct() {
    let client = make_test_client();
    let request = jsonrpc::Request::new(1_i64, "client/registerCapability", None);
    LspSaturator::handle_server_request(&client, &make_test_capability_store(), request);
}

#[tokio::test]
async fn test_handle_server_request_work_done_progress() {
    let client = make_test_client();
    let request = jsonrpc::Request::new(2_i64, "window/workDoneProgress/create", None);
    LspSaturator::handle_server_request(&client, &make_test_capability_store(), request);
}

#[tokio::test]
async fn test_handle_server_request_unknown_direct() {
    let client = make_test_client();
    let request = jsonrpc::Request::new(3_i64, "custom/unknown", None);
    LspSaturator::handle_server_request(&client, &make_test_capability_store(), request);
}

#[tokio::test]
async fn test_handle_did_open_sends_notification() {
    let client = make_test_client();
    let uri: Uri = "file:///test.rs".parse().unwrap();
    LspSaturator::handle_did_open(&client, uri, "rust".to_string(), 1, "fn main() {}".to_string());
}

#[tokio::test]
async fn test_handle_did_change_sends_notification() {
    let client = make_test_client();
    let uri: Uri = "file:///test.rs".parse().unwrap();
    LspSaturator::handle_did_change(&client, uri, 2, "fn main() { println!(); }".to_string());
}

/// Create a client whose writer channel is already dead.
///
/// Spawns `true` (exits immediately), sends a dummy message to trigger
/// the broken pipe, then waits for the writer task to exit.
async fn make_dead_client() -> Arc<Client> {
    let config = LspServerConfig {
        command: "true".to_string(),
        args: vec![],
        root_path: PathBuf::from("/tmp"),
        workspace_folders: vec![],
    };
    let (client, _reader, _stderr) = Client::spawn(config).expect("true should be available");
    let client = Arc::new(client);

    // Force the writer task to detect the broken pipe
    for _ in 0..20 {
        if client.notify("dummy", serde_json::Value::Null).is_err() {
            break;
        }
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
    }

    client
}

#[tokio::test]
async fn test_handle_did_open_with_closed_channel() {
    let client = make_dead_client().await;
    let uri: Uri = "file:///test.rs".parse().unwrap();
    // Should log error but not panic
    LspSaturator::handle_did_open(&client, uri, "rust".to_string(), 1, "fn main() {}".to_string());
}

#[tokio::test]
async fn test_handle_did_change_with_closed_channel() {
    let client = make_dead_client().await;
    let uri: Uri = "file:///test.rs".parse().unwrap();
    LspSaturator::handle_did_change(&client, uri, 2, "fn main() { println!(); }".to_string());
}

#[tokio::test]
async fn test_handle_did_close_with_closed_channel() {
    let client = make_dead_client().await;
    let cache = Arc::new(DiagnosticCache::new());
    let uri: Uri = "file:///test.rs".parse().unwrap();

    cache.store(&uri, Some(1), vec![]);
    LspSaturator::handle_did_close(&client, &cache, &uri);
    // Cache should still be cleared even if notify fails
    assert!(!cache.has(&uri));
}

#[tokio::test]
async fn test_handle_did_close_clears_cache() {
    let client = make_test_client();
    let cache = Arc::new(DiagnosticCache::new());
    let uri: Uri = "file:///test.rs".parse().unwrap();

    // Pre-populate cache
    cache.store(&uri, Some(1), vec![]);
    assert!(cache.has(&uri));

    LspSaturator::handle_did_close(&client, &cache, &uri);
    assert!(!cache.has(&uri));
}

// --- Dynamic registration tests (#533) ---

#[tokio::test]
async fn test_handle_server_request_register_capability_with_params() {
    let client = make_test_client();
    let caps = make_test_capability_store();
    assert!(caps.load_full().hover_provider.is_none());

    let params = serde_json::json!({
        "registrations": [{
            "id": "r1",
            "method": "textDocument/hover",
            "registerOptions": null
        }]
    });
    let request = jsonrpc::Request::new(1_i64, "client/registerCapability", Some(params));
    LspSaturator::handle_server_request(&client, &caps, request);

    assert!(caps.load_full().hover_provider.is_some());
}

#[tokio::test]
async fn test_handle_server_request_register_capability_no_params() {
    let client = make_test_client();
    let caps = make_test_capability_store();
    let request = jsonrpc::Request::new(1_i64, "client/registerCapability", None);
    // Should not panic, just ACK with null
    LspSaturator::handle_server_request(&client, &caps, request);
}

#[tokio::test]
async fn test_handle_server_request_register_capability_invalid_params() {
    let client = make_test_client();
    let caps = make_test_capability_store();
    let params = serde_json::json!({"invalid": "data"});
    let request = jsonrpc::Request::new(1_i64, "client/registerCapability", Some(params));
    // Should log warning but not panic
    LspSaturator::handle_server_request(&client, &caps, request);
}

#[tokio::test]
async fn test_handle_server_request_unregister_capability_with_params() {
    let client = make_test_client();
    let caps = make_test_capability_store();

    // Register first
    let reg_params = serde_json::json!({
        "registrations": [{
            "id": "r1",
            "method": "textDocument/definition",
            "registerOptions": null
        }]
    });
    let reg_request = jsonrpc::Request::new(1_i64, "client/registerCapability", Some(reg_params));
    LspSaturator::handle_server_request(&client, &caps, reg_request);
    assert!(caps.load_full().definition_provider.is_some());

    // Unregister
    let unreg_params = serde_json::json!({
        "unregisterations": [{
            "id": "r1",
            "method": "textDocument/definition"
        }]
    });
    let unreg_request =
        jsonrpc::Request::new(2_i64, "client/unregisterCapability", Some(unreg_params));
    LspSaturator::handle_server_request(&client, &caps, unreg_request);
    assert!(caps.load_full().definition_provider.is_none());
}

#[tokio::test]
async fn test_handle_server_request_unregister_capability_no_params() {
    let client = make_test_client();
    let caps = make_test_capability_store();
    let request = jsonrpc::Request::new(1_i64, "client/unregisterCapability", None);
    LspSaturator::handle_server_request(&client, &caps, request);
}

#[tokio::test]
async fn test_handle_server_request_unregister_capability_invalid_params() {
    let client = make_test_client();
    let caps = make_test_capability_store();
    let params = serde_json::json!({"bad": "format"});
    let request = jsonrpc::Request::new(1_i64, "client/unregisterCapability", Some(params));
    LspSaturator::handle_server_request(&client, &caps, request);
}

#[test]
fn test_handle_capabilities_reflects_registration() {
    let (tx, _rx) = mpsc::channel::<LspRequest>(1);
    let handle = make_test_handle(tx, true);

    // Initially no hover
    let caps = handle.capabilities().unwrap();
    assert!(caps.hover_provider.is_none());

    // Dynamically register via the shared CapabilityStore
    let reg = lsp_types::Registration {
        id: "r1".to_string(),
        method: "textDocument/hover".to_string(),
        register_options: None,
    };
    handle.capabilities.apply_registration(&reg);

    // Now capabilities() reflects the change
    let caps = handle.capabilities().unwrap();
    assert!(caps.hover_provider.is_some());
}
