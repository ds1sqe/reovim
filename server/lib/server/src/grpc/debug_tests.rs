use {
    super::*,
    crate::debug::DebugRingBuffer,
    reovim_kernel::api::v1::{Level, Record},
    std::sync::OnceLock,
};

// Test-specific ring buffer to avoid global state conflicts
static TEST_RING: OnceLock<DebugRingBuffer> = OnceLock::new();

fn test_ring() -> &'static DebugRingBuffer {
    TEST_RING.get_or_init(|| {
        let ring = DebugRingBuffer::with_capacity(4096);
        // Pre-populate with test entries using the builder pattern
        let record1 = Record::builder(Level::Info)
            .module_path("test::module")
            .file(file!())
            .line(line!())
            .message("Test message 1")
            .build();
        ring.push(&record1);

        let record2 = Record::builder(Level::Warn)
            .module_path("test::other")
            .file(file!())
            .line(line!())
            .message("Warning message")
            .build();
        ring.push(&record2);

        let record3 = Record::builder(Level::Error)
            .module_path("test::module")
            .file(file!())
            .line(line!())
            .message("Error occurred")
            .build();
        ring.push(&record3);

        ring
    })
}

#[tokio::test]
async fn test_log_level_default() {
    let service = DebugServiceImpl::new();
    let request = Request::new(LogLevelRequest { level: None });
    let response = service.log_level(request).await;

    assert!(response.is_ok());
    let resp = response.unwrap().into_inner();
    assert_eq!(resp.level, "info");
}

#[tokio::test]
async fn test_debug_service_default() {
    let service = DebugServiceImpl::default();
    let request = Request::new(LogLevelRequest { level: None });
    let response = service.log_level(request).await;
    assert!(response.is_ok());
}

#[test]
fn test_log_entry_conversion() {
    // Test that LogEntryView can be converted to proto LogEntry
    let ring = test_ring();
    let entries = ring.tail(10);

    for e in entries {
        let proto = LogEntry {
            seq: e.seq,
            timestamp_us: e.timestamp_us,
            level: e.level.to_string(),
            target: e.target.clone(),
            message: e.message.clone(),
        };
        assert!(!proto.level.is_empty());
        assert!(!proto.target.is_empty());
    }
}

#[test]
fn test_level_filter_logic() {
    let level_filter = Some("info".to_string());
    let matches_info = "INFO".eq_ignore_ascii_case(level_filter.as_ref().unwrap());
    let matches_warn = "WARN".eq_ignore_ascii_case(level_filter.as_ref().unwrap());

    assert!(matches_info);
    assert!(!matches_warn);
}

#[test]
fn test_grep_filter_logic() {
    let grep_filter = Some("error".to_string());
    let message = "Error occurred in module";

    let matches = message
        .to_lowercase()
        .contains(&grep_filter.as_ref().unwrap().to_lowercase());

    assert!(matches);
}

#[test]
fn test_target_filter_logic() {
    let target_filter = Some("module".to_string());
    let target = "test::module::submodule";

    let matches = target.contains(target_filter.as_ref().unwrap());

    assert!(matches);
}

#[tokio::test]
async fn test_log_tail_without_ring_buffer() {
    let service = DebugServiceImpl::new();
    let request = Request::new(LogTailRequest {
        count: 10,
        level: None,
        target: None,
        grep: None,
    });
    let result = service.log_tail(request).await;
    let _ = result;
}

#[tokio::test]
async fn test_log_tail_default_count() {
    let service = DebugServiceImpl::new();
    let request = Request::new(LogTailRequest {
        count: 0, // should default to 50
        level: None,
        target: None,
        grep: None,
    });
    let _ = service.log_tail(request).await;
}

#[test]
fn test_level_filter_no_match() {
    let level_filter = Some("error".to_string());
    let matches = "INFO".eq_ignore_ascii_case(level_filter.as_ref().unwrap());
    assert!(!matches);
}

#[test]
fn test_grep_filter_no_match() {
    let grep_filter = Some("fatal".to_string());
    let message = "Warning occurred";
    let matches = message
        .to_lowercase()
        .contains(&grep_filter.as_ref().unwrap().to_lowercase());
    assert!(!matches);
}

#[test]
fn test_target_filter_no_match() {
    let target_filter = Some("specific".to_string());
    let target = "other::module";
    let matches = target.contains(target_filter.as_ref().unwrap());
    assert!(!matches);
}

#[test]
fn test_debug_service_impl_default_trait() {
    fn create_default<T: Default>() -> T {
        T::default()
    }
    let _service: DebugServiceImpl = create_default();
}

/// Ensure the global ring buffer is initialized and populated for filter tests.
fn ensure_global_ring_populated() {
    let _ = crate::debug::init_debug_ring();

    if let Some(ring) = crate::debug::try_debug_ring() {
        let info_record = Record::builder(Level::Info)
            .module_path("grpc::debug::test_target")
            .file(file!())
            .line(line!())
            .message("info level coverage test message")
            .build();
        ring.push(&info_record);

        let warn_record = Record::builder(Level::Warn)
            .module_path("grpc::debug::other_target")
            .file(file!())
            .line(line!())
            .message("warn level different message")
            .build();
        ring.push(&warn_record);

        let error_record = Record::builder(Level::Error)
            .module_path("grpc::debug::test_target")
            .file(file!())
            .line(line!())
            .message("error searchable keyword")
            .build();
        ring.push(&error_record);
    }
}

#[tokio::test]
async fn test_log_tail_with_ring_buffer_no_filters() {
    ensure_global_ring_populated();

    let service = DebugServiceImpl::new();
    let request = Request::new(LogTailRequest {
        count: 10,
        level: None,
        target: None,
        grep: None,
    });
    let result = service.log_tail(request).await;
    assert!(result.is_ok());
    let response = result.unwrap().into_inner();
    assert!(!response.entries.is_empty());
}

#[tokio::test]
async fn test_log_tail_with_level_filter_match() {
    ensure_global_ring_populated();

    let service = DebugServiceImpl::new();
    let request = Request::new(LogTailRequest {
        count: 100,
        level: Some("INFO".to_string()),
        target: None,
        grep: None,
    });
    let result = service.log_tail(request).await;
    assert!(result.is_ok());
    let response = result.unwrap().into_inner();
    for entry in &response.entries {
        assert_eq!(entry.level.to_uppercase(), "INFO");
    }
}

#[tokio::test]
async fn test_log_tail_with_level_filter_excludes_others() {
    ensure_global_ring_populated();

    let service = DebugServiceImpl::new();
    let request = Request::new(LogTailRequest {
        count: 100,
        level: Some("error".to_string()),
        target: None,
        grep: None,
    });
    let result = service.log_tail(request).await;
    assert!(result.is_ok());
    let response = result.unwrap().into_inner();
    for entry in &response.entries {
        assert_eq!(entry.level.to_uppercase(), "ERROR");
    }
}

#[tokio::test]
async fn test_log_tail_with_target_filter_match() {
    ensure_global_ring_populated();

    let service = DebugServiceImpl::new();
    let request = Request::new(LogTailRequest {
        count: 100,
        level: None,
        target: Some("test_target".to_string()),
        grep: None,
    });
    let result = service.log_tail(request).await;
    assert!(result.is_ok());
    let response = result.unwrap().into_inner();
    for entry in &response.entries {
        assert!(
            entry.target.contains("test_target"),
            "target '{}' should contain 'test_target'",
            entry.target
        );
    }
}

#[tokio::test]
async fn test_log_tail_with_target_filter_excludes_others() {
    ensure_global_ring_populated();

    let service = DebugServiceImpl::new();
    let request = Request::new(LogTailRequest {
        count: 100,
        level: None,
        target: Some("nonexistent_target_xyz".to_string()),
        grep: None,
    });
    let result = service.log_tail(request).await;
    assert!(result.is_ok());
    let response = result.unwrap().into_inner();
    assert!(response.entries.is_empty(), "Should have no entries for nonexistent target");
}

#[tokio::test]
async fn test_log_tail_with_grep_filter_match() {
    ensure_global_ring_populated();

    let service = DebugServiceImpl::new();
    let request = Request::new(LogTailRequest {
        count: 100,
        level: None,
        target: None,
        grep: Some("searchable".to_string()),
    });
    let result = service.log_tail(request).await;
    assert!(result.is_ok());
    let response = result.unwrap().into_inner();
    for entry in &response.entries {
        assert!(
            entry.message.to_lowercase().contains("searchable"),
            "message '{}' should contain 'searchable'",
            entry.message
        );
    }
}

#[tokio::test]
async fn test_log_tail_with_grep_filter_excludes_others() {
    ensure_global_ring_populated();

    let service = DebugServiceImpl::new();
    let request = Request::new(LogTailRequest {
        count: 100,
        level: None,
        target: None,
        grep: Some("totally_unique_string_not_in_any_message_xyz".to_string()),
    });
    let result = service.log_tail(request).await;
    assert!(result.is_ok());
    let response = result.unwrap().into_inner();
    assert!(response.entries.is_empty(), "Should have no entries for non-matching grep");
}

#[tokio::test]
async fn test_log_tail_with_all_filters_combined() {
    ensure_global_ring_populated();

    let service = DebugServiceImpl::new();
    let request = Request::new(LogTailRequest {
        count: 100,
        level: Some("error".to_string()),
        target: Some("test_target".to_string()),
        grep: Some("searchable".to_string()),
    });
    let result = service.log_tail(request).await;
    assert!(result.is_ok());
    let response = result.unwrap().into_inner();
    for entry in &response.entries {
        assert_eq!(entry.level.to_uppercase(), "ERROR");
        assert!(entry.target.contains("test_target"));
        assert!(entry.message.to_lowercase().contains("searchable"));
    }
}

#[tokio::test]
async fn test_log_tail_grep_case_insensitive() {
    ensure_global_ring_populated();

    let service = DebugServiceImpl::new();
    let request = Request::new(LogTailRequest {
        count: 100,
        level: None,
        target: None,
        grep: Some("SEARCHABLE".to_string()),
    });
    let result = service.log_tail(request).await;
    assert!(result.is_ok());
    let response = result.unwrap().into_inner();
    for entry in &response.entries {
        assert!(entry.message.to_lowercase().contains("searchable"));
    }
}

// ── Client-targeting operation tests ──────────────────────────────────

#[test]
fn test_resolve_target_zero_rejected() {
    let err = DebugServiceImpl::resolve_target(0).unwrap_err();
    assert_eq!(err.code(), tonic::Code::InvalidArgument);
}

#[test]
fn test_resolve_target_valid() {
    let client_id = DebugServiceImpl::resolve_target(42).unwrap();
    assert_eq!(client_id.as_usize(), 42);
}

// test_debug_get_mode_no_sessions: DELETED (#753 D-chain) — DebugGetMode removed from v3

#[tokio::test]
async fn test_debug_get_projections_no_sessions() {
    let service = DebugServiceImpl::new();
    let request = Request::new(DebugGetProjectionsRequest {
        target_client_id: 1,
        tags: vec![],
    });
    let result = service.debug_get_projections(request).await;
    assert!(result.is_err());
    assert_eq!(result.unwrap_err().code(), tonic::Code::Unavailable);
}

#[tokio::test]
async fn test_debug_list_clients_no_sessions() {
    let service = DebugServiceImpl::new();
    let request = Request::new(DebugListClientsRequest {});
    let result = service.debug_list_clients(request).await;
    assert!(result.is_err());
    assert_eq!(result.unwrap_err().code(), tonic::Code::Unavailable);
}

#[test]
fn test_capture_error_to_status_no_tui() {
    let status = capture_error_to_status(CaptureError::NoTuiClient);
    assert_eq!(status.code(), tonic::Code::FailedPrecondition);
}

#[test]
fn test_capture_error_to_status_timeout() {
    let status = capture_error_to_status(CaptureError::Timeout);
    assert_eq!(status.code(), tonic::Code::DeadlineExceeded);
}

#[test]
fn test_capture_error_to_status_disconnected() {
    let status = capture_error_to_status(CaptureError::Disconnected);
    assert_eq!(status.code(), tonic::Code::Unavailable);
}

#[test]
fn test_capture_error_to_status_invalid() {
    let status = capture_error_to_status(CaptureError::InvalidResponse("bad".into()));
    assert_eq!(status.code(), tonic::Code::Internal);
}

// test_debug_get_mode_target_zero: DELETED (#753 D-chain) — DebugGetMode removed from v3

#[tokio::test]
async fn test_debug_get_projections_target_zero() {
    let service = DebugServiceImpl::new();
    let request = Request::new(DebugGetProjectionsRequest {
        target_client_id: 0,
        tags: vec![],
    });
    let result = service.debug_get_projections(request).await;
    assert!(result.is_err());
    assert_eq!(result.unwrap_err().code(), tonic::Code::InvalidArgument);
}

// ── Extension state query tests ─────────────────────────────────────

#[tokio::test]
async fn test_debug_get_extension_state_no_sessions() {
    let service = DebugServiceImpl::new();
    let request = Request::new(DebugGetExtensionStateRequest {
        kind: "whichkey".to_string(),
        target_client_id: 1,
    });
    let result = service.debug_get_extension_state(request).await;
    assert!(result.is_err());
    assert_eq!(result.unwrap_err().code(), tonic::Code::Unavailable);
}

#[tokio::test]
async fn test_debug_get_extension_state_target_zero() {
    let service = DebugServiceImpl::new();
    let request = Request::new(DebugGetExtensionStateRequest {
        kind: "whichkey".to_string(),
        target_client_id: 0,
    });
    let result = service.debug_get_extension_state(request).await;
    assert!(result.is_err());
    assert_eq!(result.unwrap_err().code(), tonic::Code::InvalidArgument);
}

#[tokio::test]
async fn test_debug_list_extensions_no_bridges() {
    let service = DebugServiceImpl::new();
    let request = Request::new(DebugListExtensionsRequest {});
    let result = service.debug_list_extensions(request).await;
    assert!(result.is_err());
    assert_eq!(result.unwrap_err().code(), tonic::Code::Unavailable);
}

// ── Tests with real sessions (covers `with_sessions()` paths) ────────

use reovim_driver_text_session::{
    ExtensionMap,
    bridges::{BridgeRegistry, ExtensionScope, ExtensionStateBridge},
};

struct TestBridge {
    kind_str: &'static str,
    bridge_scope: ExtensionScope,
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl ExtensionStateBridge for TestBridge {
    fn kind(&self) -> &'static str {
        self.kind_str
    }
    fn scope(&self) -> ExtensionScope {
        self.bridge_scope
    }
    fn snapshot(&self, _: &ExtensionMap) -> Option<serde_json::Value> {
        Some(serde_json::json!({"test": true}))
    }
    fn is_active(&self, _: &ExtensionMap) -> bool {
        true
    }
}

fn test_debug_service_with_session() -> (DebugServiceImpl, Arc<crate::session::Session>) {
    let session_id = crate::session::SessionId::new("test");
    let session = Arc::new(crate::session::Session::new(session_id.clone()));
    let sessions = Arc::new(crate::session::SessionRegistry::new());
    sessions.insert(&session);

    let mut bridges = BridgeRegistry::new();
    bridges.register(TestBridge {
        kind_str: "test-ext",
        bridge_scope: ExtensionScope::Client,
    });
    bridges.register(TestBridge {
        kind_str: "shared-ext",
        bridge_scope: ExtensionScope::Shared,
    });

    let service = DebugServiceImpl::with_sessions(sessions, session_id, Arc::new(bridges));
    (service, session)
}

// test_debug_get_mode_with_session: DELETED (#753 D-chain) — DebugGetMode removed from v3
// test_debug_get_mode_client_not_found: DELETED (#753 D-chain) — DebugGetMode removed from v3

#[tokio::test]
async fn test_debug_get_projections_with_session() {
    let (service, session) = test_debug_service_with_session();
    session.add_client(crate::session::ClientId::new(1));

    let request = Request::new(DebugGetProjectionsRequest {
        target_client_id: 1,
        tags: vec![],
    });
    let result = service.debug_get_projections(request).await;
    // Stub returns empty projections
    assert!(result.is_ok());
    assert_eq!(result.unwrap().into_inner().projections.len(), 0);
}

#[tokio::test]
async fn test_debug_list_clients_with_session() {
    let (service, session) = test_debug_service_with_session();
    session.add_client(crate::session::ClientId::new(1));
    session.add_client(crate::session::ClientId::new(2));

    let request = Request::new(DebugListClientsRequest {});
    let response = service
        .debug_list_clients(request)
        .await
        .unwrap()
        .into_inner();
    assert_eq!(response.clients.len(), 2);
}

#[tokio::test]
async fn test_debug_get_extension_state_no_bridges_configured() {
    // Construct with sessions but bridges=None
    let session_id = crate::session::SessionId::new("test");
    let session = Arc::new(crate::session::Session::new(session_id.clone()));
    let sessions = Arc::new(crate::session::SessionRegistry::new());
    sessions.insert(&session);
    let service = DebugServiceImpl {
        sessions: Some(sessions),
        default_session_id: Some(session_id),
        bridges: None,
    };

    let request = Request::new(DebugGetExtensionStateRequest {
        kind: "test-ext".to_string(),
        target_client_id: 1,
    });
    let result = service.debug_get_extension_state(request).await;
    assert!(result.is_err());
    assert_eq!(result.unwrap_err().code(), tonic::Code::Unavailable);
}

#[tokio::test]
async fn test_debug_get_extension_state_unknown_kind() {
    let (service, _) = test_debug_service_with_session();

    let request = Request::new(DebugGetExtensionStateRequest {
        kind: "nonexistent".to_string(),
        target_client_id: 1,
    });
    let result = service.debug_get_extension_state(request).await;
    assert!(result.is_err());
    assert_eq!(result.unwrap_err().code(), tonic::Code::NotFound);
}

#[tokio::test]
async fn test_debug_get_extension_state_client_scope() {
    let (service, session) = test_debug_service_with_session();
    session.add_client(crate::session::ClientId::new(1));

    let request = Request::new(DebugGetExtensionStateRequest {
        kind: "test-ext".to_string(),
        target_client_id: 1,
    });
    let response = service
        .debug_get_extension_state(request)
        .await
        .unwrap()
        .into_inner();
    assert!(response.active);
    assert!(response.data.contains("test"));
}

#[tokio::test]
async fn test_debug_get_extension_state_shared_scope() {
    // Exercises lines 425-434: the `ExtensionScope::Shared` branch in
    // `debug_get_extension_state`.  The "shared-ext" bridge is registered in
    // `test_debug_service_with_session()` with `ExtensionScope::Shared`, so
    // this request hits `session.with_state(...)` rather than the per-client
    // extensions path.  A non-zero `target_client_id` is required to pass
    // `resolve_target` even though the Shared branch does not look up a client.
    let (service, _session) = test_debug_service_with_session();

    let request = Request::new(DebugGetExtensionStateRequest {
        kind: "shared-ext".to_string(),
        target_client_id: 1,
    });
    let response = service
        .debug_get_extension_state(request)
        .await
        .unwrap()
        .into_inner();
    assert!(response.active);
    assert!(response.data.contains("test"));
}

#[tokio::test]
async fn test_debug_list_extensions_with_bridges() {
    let (service, _) = test_debug_service_with_session();

    let request = Request::new(DebugListExtensionsRequest {});
    let response = service
        .debug_list_extensions(request)
        .await
        .unwrap()
        .into_inner();
    assert_eq!(response.extensions.len(), 2);
    let scopes: Vec<&str> = response
        .extensions
        .iter()
        .map(|e| e.scope.as_str())
        .collect();
    assert!(scopes.contains(&"client"));
    assert!(scopes.contains(&"shared"));
}

#[tokio::test]
async fn test_debug_list_extensions_no_bridges_configured() {
    let session_id = crate::session::SessionId::new("test");
    let session = Arc::new(crate::session::Session::new(session_id.clone()));
    let sessions = Arc::new(crate::session::SessionRegistry::new());
    sessions.insert(&session);
    let service = DebugServiceImpl {
        sessions: Some(sessions),
        default_session_id: Some(session_id),
        bridges: None,
    };

    let request = Request::new(DebugListExtensionsRequest {});
    let result = service.debug_list_extensions(request).await;
    assert!(result.is_err());
    assert_eq!(result.unwrap_err().code(), tonic::Code::Unavailable);
}
