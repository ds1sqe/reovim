use super::*;
use crate::SelectionState;

// Mock context for testing
struct MockContext {
    state: TuiCoreState,
    buffer_modified_calls: Vec<u64>,
    extensions: Vec<Box<dyn ClientModule>>,
}

impl MockContext {
    fn new(client_id: u64) -> Self {
        Self {
            state: TuiCoreState::new(client_id),
            buffer_modified_calls: Vec::new(),
            extensions: Vec::new(),
        }
    }

    fn with_extensions(mut self, extensions: Vec<Box<dyn ClientModule>>) -> Self {
        self.extensions = extensions;
        self
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl NotificationContext for MockContext {
    fn state_mut(&mut self) -> &mut TuiCoreState {
        &mut self.state
    }

    fn client_mut(&mut self) -> &mut TuiGrpcClient {
        // This would panic in tests that try to use the client
        // For unit tests, we avoid calling methods that need the client
        unimplemented!("Mock context doesn't have a real client")
    }

    fn on_buffer_modified(&mut self, buffer_id: u64) {
        self.buffer_modified_calls.push(buffer_id);
    }

    fn extensions_mut(&mut self) -> &mut [Box<dyn ClientModule>] {
        &mut self.extensions
    }
}

#[test]
fn test_notification_result_variants() {
    // Just ensure the variants exist
    let _ = NotificationResult::Redraw;
    let _ = NotificationResult::NoRedraw;
    let _ = NotificationResult::Stop;
}

#[test]
fn test_mock_context() {
    let mut ctx = MockContext::new(1);
    assert_eq!(ctx.state_mut().my_client_id, 1);

    ctx.on_buffer_modified(42);
    assert_eq!(ctx.buffer_modified_calls, vec![42]);
}

// =========================================================================
// Default trait method tests
// =========================================================================

#[test]
fn test_default_on_buffer_modified_is_noop() {
    // Ensure the default trait implementation doesn't panic
    struct MinimalContext {
        state: TuiCoreState,
        extensions: Vec<Box<dyn ClientModule>>,
    }
    #[cfg_attr(coverage_nightly, coverage(off))]
    impl NotificationContext for MinimalContext {
        fn state_mut(&mut self) -> &mut TuiCoreState {
            &mut self.state
        }
        fn client_mut(&mut self) -> &mut crate::grpc_client::TuiGrpcClient {
            unimplemented!()
        }
        fn extensions_mut(&mut self) -> &mut [Box<dyn ClientModule>] {
            &mut self.extensions
        }
    }

    let mut ctx = MinimalContext {
        state: TuiCoreState::new(1),
        extensions: Vec::new(),
    };
    // Default trait methods should be no-ops and not panic
    ctx.on_buffer_modified(42);
    assert!(ctx.on_capture_request(1, "plain_text", 1).is_none());
    ctx.on_resize(80, 24);
}

// =========================================================================
// Synchronous notification dispatch tests (no client needed)
// =========================================================================

fn make_notif(payload: reovim_protocol::v3::notification::Payload) -> Notification {
    Notification {
        event_type: String::new(),
        timestamp_ms: 0,
        payload: Some(payload),
    }
}

#[tokio::test]
async fn test_handle_empty_payload() {
    let mut ctx = MockContext::new(1);
    let notif = Notification {
        event_type: String::new(),
        timestamp_ms: 0,
        payload: None,
    };
    let result = handle_notification(&mut ctx, notif).await.unwrap();
    assert!(matches!(result, NotificationResult::NoRedraw));
}

#[tokio::test]
async fn test_handle_layout_changed() {
    use reovim_protocol::v3::{LayoutChangedPayload, WindowInfo};

    let mut ctx = MockContext::new(1);
    // Pre-populate buffer cache so the handler doesn't try to fetch
    // (MockContext.client_mut() panics).
    ctx.state
        .buffer_cache
        .insert(100, vec!["line1".to_string()]);
    ctx.state
        .buffer_cache
        .insert(200, vec!["line2".to_string()]);

    let notif = make_notif(Payload::LayoutChanged(LayoutChangedPayload {
        focused_window_id: Some(3),
        windows: vec![
            WindowInfo {
                window_id: 3,
                buffer_id: Some(100),
                rect: None,
                focused: true,
                opacity: None,
                primary_domain_id: 0,
                embedded_domain_ids: vec![],
                spatial_placement: None,
            },
            WindowInfo {
                window_id: 4,
                buffer_id: Some(200),
                rect: None,
                focused: false,
                opacity: None,
                primary_domain_id: 0,
                embedded_domain_ids: vec![],
                spatial_placement: None,
            },
        ],
        client_id: 1,
        active_tab_id: None,
        tabs: Vec::new(),
    }));

    let result = handle_notification(&mut ctx, notif).await.unwrap();
    assert!(matches!(result, NotificationResult::RedrawWithMetadata));
    assert_eq!(ctx.state.focused_window_id, 3);
    assert_eq!(ctx.state.windows.len(), 2);
}

#[tokio::test]
async fn test_handle_render_complete() {
    use reovim_protocol::v3::RenderCompletePayload;

    let mut ctx = MockContext::new(1);
    let notif = make_notif(Payload::RenderComplete(RenderCompletePayload { frame_id: 42 }));

    let result = handle_notification(&mut ctx, notif).await.unwrap();
    assert!(matches!(result, NotificationResult::Redraw));
}

#[tokio::test]
async fn test_handle_detach() {
    use reovim_protocol::v3::DetachPayload;

    let mut ctx = MockContext::new(1);
    let notif = make_notif(Payload::Detach(DetachPayload {
        reason: "server shutting down".to_string(),
    }));

    let result = handle_notification(&mut ctx, notif).await.unwrap();
    assert!(matches!(result, NotificationResult::Stop));
}

#[tokio::test]
async fn test_handle_presence_joined_remote() {
    use reovim_protocol::v3::{ClientPresence, PresenceJoinedPayload};

    let mut ctx = MockContext::new(1);
    let notif = make_notif(Payload::PresenceJoined(PresenceJoinedPayload {
        client: Some(ClientPresence {
            client_id: 5,
            client_type: "tui".to_string(),
            display_name: "laptop".to_string(),
            buffer_id: Some(100),
            viewport_state: None,
            spatial_state: None,
            sync_mode: 0,
            follow_target: None,
            joined_at_ms: 0,
        }),
    }));

    let result = handle_notification(&mut ctx, notif).await.unwrap();
    assert!(matches!(result, NotificationResult::Redraw));
    assert!(ctx.state.other_clients.contains_key(&5));
    assert_eq!(ctx.state.other_clients[&5].display_name, "laptop");
    // Mode defaults to empty in v3 until ProjectionUpdated arrives
    assert!(ctx.state.other_clients[&5].mode.is_empty());
}

#[tokio::test]
async fn test_handle_presence_joined_self_ignored() {
    use reovim_protocol::v3::{ClientPresence, PresenceJoinedPayload};

    let mut ctx = MockContext::new(1);
    let notif = make_notif(Payload::PresenceJoined(PresenceJoinedPayload {
        client: Some(ClientPresence {
            client_id: 1, // Same as our ID
            client_type: "tui".to_string(),
            display_name: "self".to_string(),
            buffer_id: Some(100),
            viewport_state: None,
            spatial_state: None,
            sync_mode: 0,
            follow_target: None,
            joined_at_ms: 0,
        }),
    }));

    let result = handle_notification(&mut ctx, notif).await.unwrap();
    assert!(matches!(result, NotificationResult::Redraw));
    assert!(!ctx.state.other_clients.contains_key(&1));
}

#[tokio::test]
async fn test_handle_presence_joined_no_client() {
    use reovim_protocol::v3::PresenceJoinedPayload;

    let mut ctx = MockContext::new(1);
    let notif = make_notif(Payload::PresenceJoined(PresenceJoinedPayload { client: None }));

    let result = handle_notification(&mut ctx, notif).await.unwrap();
    assert!(matches!(result, NotificationResult::Redraw));
    assert!(ctx.state.other_clients.is_empty());
}

#[tokio::test]
async fn test_handle_presence_updated_buffer_changed() {
    use reovim_protocol::v3::{ClientPresence, PresenceUpdatedPayload};

    let mut ctx = MockContext::new(1);
    // First add the remote client
    ctx.state.add_remote_client(RemoteClient {
        client_id: 2,
        display_name: "remote".to_string(),
        cursor_line: 10,
        cursor_col: 5,
        buffer_id: Some(100),
        mode: "NORMAL".to_string(),
        selection: Some(SelectionState::default()),
    });

    // Update with different buffer_id => cursor and mode reset
    let notif = make_notif(Payload::PresenceUpdated(PresenceUpdatedPayload {
        client: Some(ClientPresence {
            client_id: 2,
            client_type: "tui".to_string(),
            display_name: "remote".to_string(),
            buffer_id: Some(200), // Different buffer
            viewport_state: None,
            spatial_state: None,
            sync_mode: 0,
            follow_target: None,
            joined_at_ms: 0,
        }),
    }));

    let result = handle_notification(&mut ctx, notif).await.unwrap();
    assert!(matches!(result, NotificationResult::Redraw));
    let remote = &ctx.state.other_clients[&2];
    assert_eq!(remote.cursor_line, 0); // Reset
    assert_eq!(remote.cursor_col, 0); // Reset
    assert!(remote.selection.is_none()); // Cleared
    assert_eq!(remote.buffer_id, Some(200));
    assert!(remote.mode.is_empty()); // Reset for new buffer
}

#[tokio::test]
async fn test_handle_presence_updated_same_buffer_preserves_cursor() {
    use reovim_protocol::v3::{ClientPresence, PresenceUpdatedPayload};

    let mut ctx = MockContext::new(1);
    ctx.state.add_remote_client(RemoteClient {
        client_id: 2,
        display_name: "remote".to_string(),
        cursor_line: 10,
        cursor_col: 5,
        buffer_id: Some(100),
        mode: "NORMAL".to_string(),
        selection: None,
    });

    // Update with same buffer_id => cursor preserved
    let notif = make_notif(Payload::PresenceUpdated(PresenceUpdatedPayload {
        client: Some(ClientPresence {
            client_id: 2,
            client_type: "tui".to_string(),
            display_name: "remote-updated".to_string(),
            buffer_id: Some(100), // Same buffer
            viewport_state: None,
            spatial_state: None,
            sync_mode: 0,
            follow_target: None,
            joined_at_ms: 0,
        }),
    }));

    let result = handle_notification(&mut ctx, notif).await.unwrap();
    assert!(matches!(result, NotificationResult::Redraw));
    let remote = &ctx.state.other_clients[&2];
    assert_eq!(remote.cursor_line, 10); // Preserved
    assert_eq!(remote.cursor_col, 5); // Preserved
    assert_eq!(remote.display_name, "remote-updated");
}

#[tokio::test]
async fn test_handle_presence_updated_self_ignored() {
    use reovim_protocol::v3::{ClientPresence, PresenceUpdatedPayload};

    let mut ctx = MockContext::new(1);
    let notif = make_notif(Payload::PresenceUpdated(PresenceUpdatedPayload {
        client: Some(ClientPresence {
            client_id: 1, // Self
            client_type: "tui".to_string(),
            display_name: "self".to_string(),
            buffer_id: Some(100),
            viewport_state: None,
            spatial_state: None,
            sync_mode: 0,
            follow_target: None,
            joined_at_ms: 0,
        }),
    }));

    let result = handle_notification(&mut ctx, notif).await.unwrap();
    assert!(matches!(result, NotificationResult::Redraw));
    assert!(ctx.state.other_clients.is_empty());
}

#[tokio::test]
async fn test_handle_presence_updated_no_client() {
    use reovim_protocol::v3::PresenceUpdatedPayload;

    let mut ctx = MockContext::new(1);
    let notif = make_notif(Payload::PresenceUpdated(PresenceUpdatedPayload { client: None }));

    let result = handle_notification(&mut ctx, notif).await.unwrap();
    assert!(matches!(result, NotificationResult::Redraw));
}

#[tokio::test]
async fn test_handle_presence_left() {
    use reovim_protocol::v3::PresenceLeftPayload;

    let mut ctx = MockContext::new(1);
    ctx.state.add_remote_client(RemoteClient {
        client_id: 5,
        display_name: "gone".to_string(),
        cursor_line: 0,
        cursor_col: 0,
        buffer_id: Some(1),
        mode: "NORMAL".to_string(),
        selection: None,
    });
    assert!(ctx.state.other_clients.contains_key(&5));

    let notif = make_notif(Payload::PresenceLeft(PresenceLeftPayload {
        client_id: 5,
        display_name: "gone".to_string(),
    }));

    let result = handle_notification(&mut ctx, notif).await.unwrap();
    assert!(matches!(result, NotificationResult::Redraw));
    assert!(!ctx.state.other_clients.contains_key(&5));
}

#[tokio::test]
async fn test_handle_capture_request_different_client() {
    use reovim_protocol::v3::CaptureRequestPayload;

    let mut ctx = MockContext::new(1);
    let notif = make_notif(Payload::CaptureRequest(CaptureRequestPayload {
        request_id: 42,
        format: "plain_text".to_string(),
        target_client_id: 99, // Different client
    }));

    let result = handle_notification(&mut ctx, notif).await.unwrap();
    assert!(matches!(result, NotificationResult::NoRedraw));
}

#[tokio::test]
async fn test_handle_capture_request_for_us_no_handler() {
    use reovim_protocol::v3::CaptureRequestPayload;

    let mut ctx = MockContext::new(1);
    // MockContext::on_capture_request returns None by default
    let notif = make_notif(Payload::CaptureRequest(CaptureRequestPayload {
        request_id: 42,
        format: "plain_text".to_string(),
        target_client_id: 1,
    }));

    let result = handle_notification(&mut ctx, notif).await.unwrap();
    assert!(matches!(result, NotificationResult::NoRedraw));
}

// =========================================================================
// ProjectionUpdated dispatch tests (new in v3)
// =========================================================================

#[tokio::test]
async fn test_handle_projection_updated_text_mode() {
    use reovim_protocol::v3::{DomainDatum, ProjectionUpdatedPayload};

    let mut ctx = MockContext::new(1);
    let notif = make_notif(Payload::ProjectionUpdated(ProjectionUpdatedPayload {
        tag: "text.mode".to_string(),
        domain_id: 1,
        window_id: None,
        datum: Some(DomainDatum {
            content: b"INSERT".to_vec(),
            display: None,
        }),
        transient: false,
        version: 1,
        client_id: 1,
    }));

    let result = handle_notification(&mut ctx, notif).await.unwrap();
    assert!(matches!(result, NotificationResult::Redraw));
    assert_eq!(ctx.state.mode_name, "INSERT");
    assert!(ctx.state.is_insert_mode());
}

#[tokio::test]
async fn test_handle_projection_updated_unknown_tag_no_panic() {
    use reovim_protocol::v3::{DomainDatum, ProjectionUpdatedPayload};

    let mut ctx = MockContext::new(1);
    let notif = make_notif(Payload::ProjectionUpdated(ProjectionUpdatedPayload {
        tag: "mesh.camera".to_string(),
        domain_id: 99,
        window_id: None,
        datum: Some(DomainDatum {
            content: vec![0xFF],
            display: None,
        }),
        transient: false,
        version: 0,
        client_id: 1,
    }));

    // Unknown tag should not panic
    let result = handle_notification(&mut ctx, notif).await.unwrap();
    assert!(matches!(result, NotificationResult::Redraw));
}

// =========================================================================
// SurfaceChanged dispatch tests (replaces ResizeRequest in v3)
// =========================================================================

#[tokio::test]
async fn test_handle_surface_changed_cell_grid() {
    use reovim_protocol::v3::{SurfaceChangedPayload, SurfaceDescriptorProto};

    let mut ctx = MockContext::new(1);

    // Encode CellGridSurface: kind=0x0001, width=120_u32_be, height=40_u32_be
    let mut body = Vec::new();
    body.extend_from_slice(&120_u32.to_be_bytes());
    body.extend_from_slice(&40_u32.to_be_bytes());

    let notif = make_notif(Payload::SurfaceChanged(SurfaceChangedPayload {
        surface: Some(SurfaceDescriptorProto { kind: 0x0001, body }),
        target_client_id: 1,
    }));

    let result = handle_notification(&mut ctx, notif).await.unwrap();
    assert!(matches!(result, NotificationResult::Redraw));
    assert_eq!(ctx.state.width, 120);
    assert_eq!(ctx.state.height, 40);
}

#[tokio::test]
async fn test_handle_surface_changed_different_client_ignored() {
    use reovim_protocol::v3::{SurfaceChangedPayload, SurfaceDescriptorProto};

    let mut ctx = MockContext::new(1);

    let mut body = Vec::new();
    body.extend_from_slice(&100_u32.to_be_bytes());
    body.extend_from_slice(&30_u32.to_be_bytes());

    let notif = make_notif(Payload::SurfaceChanged(SurfaceChangedPayload {
        surface: Some(SurfaceDescriptorProto { kind: 0x0001, body }),
        target_client_id: 99, // Different client
    }));

    let result = handle_notification(&mut ctx, notif).await.unwrap();
    assert!(matches!(result, NotificationResult::NoRedraw));
    // State should NOT be updated
    assert_eq!(ctx.state.width, 0);
    assert_eq!(ctx.state.height, 0);
}

#[tokio::test]
async fn test_handle_surface_changed_unknown_kind_ignored() {
    use reovim_protocol::v3::{SurfaceChangedPayload, SurfaceDescriptorProto};

    let mut ctx = MockContext::new(1);
    ctx.state.width = 80;
    ctx.state.height = 24;

    let notif = make_notif(Payload::SurfaceChanged(SurfaceChangedPayload {
        surface: Some(SurfaceDescriptorProto {
            kind: 0xFFFF, // Unknown kind
            body: vec![0x00; 8],
        }),
        target_client_id: 1,
    }));

    let result = handle_notification(&mut ctx, notif).await.unwrap();
    assert!(matches!(result, NotificationResult::Redraw));
    // Dimensions should NOT change for unknown kind
    assert_eq!(ctx.state.width, 80);
    assert_eq!(ctx.state.height, 24);
}

// =========================================================================
// ExtensionUpdated generic dispatch tests (#468)
// =========================================================================

// Test extension stub — engine has ZERO knowledge of real extensions.
// This verifies the generic dispatch mechanism only.
use std::sync::{Arc, Mutex};

use reovim_client_driver::{ClientModuleError, ProbeResult, Version};

/// Shared handle to inspect what data a `StubExtension` received.
#[derive(Clone)]
struct StubHandle(Arc<Mutex<String>>);

impl StubHandle {
    fn new() -> Self {
        Self(Arc::new(Mutex::new(String::new())))
    }

    fn received_data(&self) -> bool {
        !self.0.lock().unwrap().is_empty()
    }
}

struct StubExtension {
    ext_kind: &'static str,
    shared: StubHandle,
}

impl StubExtension {
    fn new(kind: &'static str) -> (Self, StubHandle) {
        let handle = StubHandle::new();
        (
            Self {
                ext_kind: kind,
                shared: handle.clone(),
            },
            handle,
        )
    }
}

impl ClientModule for StubExtension {
    fn id(&self) -> &'static str {
        self.ext_kind
    }
    fn kind(&self) -> &'static str {
        self.ext_kind
    }
    fn name(&self) -> &'static str {
        "Stub"
    }
    fn version(&self) -> Version {
        Version::new(0, 1, 0)
    }
    fn init(&mut self, _ctx: &reovim_client_driver::ModuleContext) -> ProbeResult {
        ProbeResult::Success
    }
    fn exit(&mut self) -> Result<(), ClientModuleError> {
        Ok(())
    }

    fn on_notification(&mut self, data: &str) {
        *self.shared.0.lock().unwrap() = data.to_string();
    }
}

#[tokio::test]
async fn test_handle_extension_updated_dispatches_to_matching() {
    use reovim_protocol::v3::ExtensionUpdatedPayload;

    let (cmdline, cmdline_h) = StubExtension::new("cmdline");
    let (whichkey, whichkey_h) = StubExtension::new("whichkey");
    let mut ctx = MockContext::new(1).with_extensions(vec![Box::new(cmdline), Box::new(whichkey)]);

    let notif = make_notif(Payload::ExtensionUpdated(ExtensionUpdatedPayload {
        kind: "cmdline".to_string(),
        data: r#"{"active":true,"prompt":":","input":"wq","cursor":2}"#.to_string(),
        client_id: 1,
    }));

    let result = handle_notification(&mut ctx, notif).await.unwrap();
    assert!(matches!(result, NotificationResult::Redraw));
    // The matching extension should have received data
    assert!(cmdline_h.received_data());
    // The other extension should NOT be affected
    assert!(!whichkey_h.received_data());
}

#[tokio::test]
async fn test_handle_extension_updated_remote_ignored() {
    use reovim_protocol::v3::ExtensionUpdatedPayload;

    let (cmdline, cmdline_h) = StubExtension::new("cmdline");
    let mut ctx = MockContext::new(1).with_extensions(vec![Box::new(cmdline)]);

    let notif = make_notif(Payload::ExtensionUpdated(ExtensionUpdatedPayload {
        kind: "cmdline".to_string(),
        data: r#"{"active":true}"#.to_string(),
        client_id: 99, // Different client
    }));

    let result = handle_notification(&mut ctx, notif).await.unwrap();
    assert!(matches!(result, NotificationResult::Redraw));
    // Extension should NOT be updated (remote client)
    assert!(!cmdline_h.received_data());
}

#[tokio::test]
async fn test_handle_extension_updated_unknown_kind_ignored() {
    use reovim_protocol::v3::ExtensionUpdatedPayload;

    let (cmdline, cmdline_h) = StubExtension::new("cmdline");
    let (whichkey, whichkey_h) = StubExtension::new("whichkey");
    let mut ctx = MockContext::new(1).with_extensions(vec![Box::new(cmdline), Box::new(whichkey)]);

    let notif = make_notif(Payload::ExtensionUpdated(ExtensionUpdatedPayload {
        kind: "unknown_ext".to_string(),
        data: r#"{"active":true}"#.to_string(),
        client_id: 1,
    }));

    let result = handle_notification(&mut ctx, notif).await.unwrap();
    assert!(matches!(result, NotificationResult::Redraw));
    // Neither extension should be affected
    assert!(!cmdline_h.received_data());
    assert!(!whichkey_h.received_data());
}

#[tokio::test]
async fn test_handle_extension_updated_client_id_zero_is_local() {
    use reovim_protocol::v3::ExtensionUpdatedPayload;

    let (cmdline, cmdline_h) = StubExtension::new("cmdline");
    let mut ctx = MockContext::new(1).with_extensions(vec![Box::new(cmdline)]);

    let notif = make_notif(Payload::ExtensionUpdated(ExtensionUpdatedPayload {
        kind: "cmdline".to_string(),
        data: r#"{"active":true}"#.to_string(),
        client_id: 0, // 0 means local/unspecified
    }));

    let result = handle_notification(&mut ctx, notif).await.unwrap();
    assert!(matches!(result, NotificationResult::Redraw));
    // Extension should be updated (client_id 0 treated as local)
    assert!(cmdline_h.received_data());
}

#[tokio::test]
async fn test_handle_extension_updated_invalid_json_no_panic() {
    use reovim_protocol::v3::ExtensionUpdatedPayload;

    let (cmdline, cmdline_h) = StubExtension::new("cmdline");
    let mut ctx = MockContext::new(1).with_extensions(vec![Box::new(cmdline)]);

    let notif = make_notif(Payload::ExtensionUpdated(ExtensionUpdatedPayload {
        kind: "cmdline".to_string(),
        data: "not valid json{{{".to_string(),
        client_id: 1,
    }));

    // Should not panic — extensions handle their own JSON parsing
    let result = handle_notification(&mut ctx, notif).await.unwrap();
    assert!(matches!(result, NotificationResult::Redraw));
    // Extension received data (even invalid JSON — it's up to the module to parse)
    assert!(cmdline_h.received_data());
}

#[tokio::test]
async fn test_handle_extension_updated_no_extensions() {
    use reovim_protocol::v3::ExtensionUpdatedPayload;

    // No extensions registered — should not panic
    let mut ctx = MockContext::new(1);
    let notif = make_notif(Payload::ExtensionUpdated(ExtensionUpdatedPayload {
        kind: "cmdline".to_string(),
        data: r#"{"active":true}"#.to_string(),
        client_id: 1,
    }));

    let result = handle_notification(&mut ctx, notif).await.unwrap();
    assert!(matches!(result, NotificationResult::Redraw));
}

#[tokio::test]
async fn test_notification_result_debug() {
    let redraw = NotificationResult::Redraw;
    let no_redraw = NotificationResult::NoRedraw;
    let stop = NotificationResult::Stop;
    assert!(format!("{redraw:?}").contains("Redraw"));
    assert!(format!("{no_redraw:?}").contains("NoRedraw"));
    assert!(format!("{stop:?}").contains("Stop"));
}
