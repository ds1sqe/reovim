use {super::*, crate::session::SessionId, reovim_subsys_session::change_set::ChangeSet};

#[test]
fn test_current_timestamp_ms() {
    let ts = current_timestamp_ms();
    // Should be a reasonable recent timestamp (after 2024)
    assert!(ts > 1_700_000_000_000);
}

#[test]
fn test_build_notifications_empty_changes() {
    let changes = ChangeSet::new();
    let session = Session::new(SessionId::new("test"));
    // Use client_id 0 - no client registered, so per-client lookups return None
    // and fallback to shared state (or return empty notifications)
    let notifications = build_notifications(&changes, &session, 0, None);
    assert!(notifications.is_empty());
}

// test_build_buffer_modified_notification: REMOVED — build_buffer_modified_notification deleted in v3 (#753).

// test_build_buffer_list_notification_added: REMOVED — build_buffer_list_notification deleted in v3 (#753).
// test_build_buffer_list_notification_removed: REMOVED — build_buffer_list_notification deleted in v3 (#753).

// test_build_notifications_buffer_modified: REMOVED — BufferModified notification deleted in v3 (#753).

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_build_notifications_buffer_created() {
    let buffer_id = reovim_kernel::api::v1::BufferId::from_raw(10);
    let mut changes = ChangeSet::new();
    changes.created_buffers.push(buffer_id);

    let session = Session::new(SessionId::new("buf-create-test"));
    let notifications = build_notifications(&changes, &session, 0, None);

    assert_eq!(notifications.len(), 1);
    assert_eq!(notifications[0].event_type, "buffer_opened");

    if let Some(notification::Payload::BufferOpened(payload)) = &notifications[0].payload {
        assert_eq!(payload.buffer_id, 10);
    } else {
        panic!("Expected BufferOpenedPayload");
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_build_notifications_buffer_deleted() {
    let buffer_id = reovim_kernel::api::v1::BufferId::from_raw(20);
    let mut changes = ChangeSet::new();
    changes.deleted_buffers.push(buffer_id);

    let session = Session::new(SessionId::new("buf-delete-test"));
    let notifications = build_notifications(&changes, &session, 0, None);

    assert_eq!(notifications.len(), 1);
    assert_eq!(notifications[0].event_type, "buffer_closed");

    if let Some(notification::Payload::BufferClosed(payload)) = &notifications[0].payload {
        assert_eq!(payload.buffer_id, 20);
    } else {
        panic!("Expected BufferClosedPayload");
    }
}

#[test]
fn test_build_notifications_window_changed() {
    let mut changes = ChangeSet::new();
    changes.layout_changed = true;

    let session = Session::new(SessionId::new("win-test"));
    let notifications = build_notifications(&changes, &session, 0, None);

    assert_eq!(notifications.len(), 1);
    assert_eq!(notifications[0].event_type, "layout_changed");
}

#[test]
fn test_build_notifications_focus_changed() {
    let mut changes = ChangeSet::new();
    changes.focus_changed = true;

    let session = Session::new(SessionId::new("focus-test"));
    let notifications = build_notifications(&changes, &session, 0, None);

    assert_eq!(notifications.len(), 1);
    assert_eq!(notifications[0].event_type, "layout_changed");
}

#[test]
fn test_build_notifications_cursor_moved_no_client() {
    let buffer_id = reovim_kernel::api::v1::BufferId::from_raw(1);
    let mut changes = ChangeSet::new();
    changes.record_cursor_move(buffer_id);

    let session = Session::new(SessionId::new("cursor-test"));
    // No client registered, so cursor notifications are skipped
    let notifications = build_notifications(&changes, &session, 0, None);

    // Should have no notifications because cursor notification returns None for unknown client
    assert!(notifications.is_empty());
}

#[test]
fn test_build_notifications_selection_changed_no_client() {
    let buffer_id = reovim_kernel::api::v1::BufferId::from_raw(1);
    let mut changes = ChangeSet::new();
    changes.selection_changed = true;
    changes.affected_buffers.push(buffer_id);

    let session = Session::new(SessionId::new("sel-test"));
    // No client registered
    let notifications = build_notifications(&changes, &session, 0, None);

    // No selection notification because client doesn't exist
    assert!(notifications.is_empty());
}

#[test]
fn test_build_notifications_scroll_changed_no_client() {
    let window_id = reovim_kernel::api::v1::WindowId::from_raw(1);
    let mut changes = ChangeSet::new();
    changes.scroll_changed = true;
    changes.scrolled_windows.push(window_id);

    let session = Session::new(SessionId::new("scroll-test"));
    let notifications = build_notifications(&changes, &session, 0, None);

    // No viewport notification because client doesn't exist
    assert!(notifications.is_empty());
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_build_layout_notification_no_compositor_no_client() {
    let session = Session::new(SessionId::new("layout-test"));
    let notification = build_layout_notification(&session, 12345, 999);

    assert_eq!(notification.event_type, "layout_changed");
    if let Some(notification::Payload::LayoutChanged(payload)) = notification.payload {
        // No client and no compositor - should produce empty windows
        assert!(payload.windows.is_empty());
        assert!(payload.focused_window_id.is_none());
    } else {
        panic!("Expected LayoutChangedPayload");
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_build_notifications_multiple_buffers_created_and_deleted() {
    let buf1 = reovim_kernel::api::v1::BufferId::from_raw(1);
    let buf2 = reovim_kernel::api::v1::BufferId::from_raw(2);
    let buf3 = reovim_kernel::api::v1::BufferId::from_raw(3);

    let mut changes = ChangeSet::new();
    changes.created_buffers.push(buf1);
    changes.created_buffers.push(buf2);
    changes.deleted_buffers.push(buf3);

    let session = Session::new(SessionId::new("multi-buf-test"));
    let notifications = build_notifications(&changes, &session, 0, None);

    // 2 created + 1 deleted = 3 notifications (buffer_opened / buffer_closed)
    assert_eq!(notifications.len(), 3);

    let mut opened_count = 0;
    let mut closed_count = 0;
    for n in &notifications {
        match &n.payload {
            Some(notification::Payload::BufferOpened(_)) => opened_count += 1,
            Some(notification::Payload::BufferClosed(_)) => closed_count += 1,
            _ => {}
        }
    }
    assert_eq!(opened_count, 2);
    assert_eq!(closed_count, 1);
}

// test_build_layout_notification_with_client_windows removed:
// per-client windows fallback is domain-owned (#753 E3).
// Without compositor, build_layout_notification returns empty windows list.
#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_build_layout_notification_no_compositor_empty_windows() {
    use reovim_kernel::api::v1::{ModeId, ModeStack, ModuleId};

    let session = Session::new(SessionId::new("layout-client-test"));
    let client_id = crate::session::ClientId::new(9);

    let mode = ModeId::new(ModuleId::new("test"), "normal");
    let mode_stack = ModeStack::new(mode);
    let metadata = crate::session::ClientMetadata::default();
    // No compositor set; windows are domain-owned (#753 E3)
    let client = crate::session::Client::with_mode_stack(client_id, metadata, mode_stack);
    session.clients().add_client_with_state(client);

    let notification = build_layout_notification(&session, 88888, 9);
    assert_eq!(notification.event_type, "layout_changed");
    if let Some(notification::Payload::LayoutChanged(payload)) = notification.payload {
        // No compositor -> empty windows (domain driver provides layout, #753 E3)
        assert_eq!(payload.windows.len(), 0);
    } else {
        panic!("Expected LayoutChangedPayload");
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
#[allow(clippy::too_many_lines)]
fn test_build_layout_notification_with_compositor() {
    use reovim_subsys_layout::{
        CompositeResult, Layer, LayerConfig, LayerId, Rect, RootCompositor, WindowId,
        WindowLayerCompositor, WindowPlacement, ZOrder, Zone,
    };

    // Mock compositor that returns real placements
    struct TestCompositor {
        focused: Option<WindowId>,
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    impl RootCompositor for TestCompositor {
        fn composite(&self, screen: Rect) -> CompositeResult {
            let placement = WindowPlacement::new(
                WindowId::from_raw(1),
                LayerId::new(0),
                Zone::Tiled,
                Rect::new(0, 0, screen.width, screen.height),
                ZOrder::new(100),
            );
            CompositeResult {
                placements: vec![placement],
                focused: self.focused,
                active_layer: Some(LayerId::new(0)),
                screen,
            }
        }

        fn create_layer(&mut self, _config: LayerConfig) -> LayerId {
            LayerId::new(0)
        }
        fn remove_layer(&mut self, _layer: LayerId) {}
        fn layer_by_label(&self, _label: &str) -> Option<LayerId> {
            None
        }
        fn layers(&self) -> Vec<&Layer> {
            Vec::new()
        }
        fn set_layer_visible(&mut self, _layer: LayerId, _visible: bool) {}
        fn set_layer_opacity(&mut self, _layer: LayerId, _opacity: f32) {}
        fn reorder_layer(&mut self, _layer: LayerId, _new_z: u16) {}
        fn set_active_layer(&mut self, _layer: LayerId) {}
        fn active_layer(&self) -> Option<LayerId> {
            Some(LayerId::new(0))
        }
        fn set_focus(&mut self, window: WindowId) {
            self.focused = Some(window);
        }
        fn focused(&self) -> Option<WindowId> {
            self.focused
        }
        fn focus_at(&mut self, _x: u16, _y: u16) -> Option<WindowId> {
            self.focused
        }
        fn layer_compositor(&self, _layer: LayerId) -> Option<&dyn WindowLayerCompositor> {
            None
        }
        fn layer_compositor_mut(
            &mut self,
            _layer: LayerId,
        ) -> Option<&mut dyn WindowLayerCompositor> {
            None
        }
        fn window_count(&self) -> usize {
            1
        }
        fn set_screen(&mut self, _screen: Rect) {}
        fn layer_of(&self, _window: WindowId) -> Option<LayerId> {
            Some(LayerId::new(0))
        }
        fn boxed_clone(&self) -> Box<dyn RootCompositor> {
            Box::new(Self {
                focused: self.focused,
            })
        }
        fn generation(&self) -> u64 {
            0
        }
        fn topology(&self) -> reovim_subsys_layout::LayoutTopology {
            reovim_subsys_layout::LayoutTopology::Single(WindowId::from_raw(1))
        }
    }

    // Create a session state (compositor is now per-client, not shared)
    let state = crate::session::SessionState::default();

    let session = Session::from_state(SessionId::new("compositor-test"), state);

    // Add a client with compositor; windows are domain-owned (#753 E3)
    let client_id = crate::session::ClientId::new(1);
    let mode = reovim_kernel::api::v1::ModeId::new(
        reovim_kernel::api::v1::ModuleId::new("test"),
        "normal",
    );
    let mode_stack = reovim_kernel::api::v1::ModeStack::new(mode);
    let metadata = crate::session::ClientMetadata::default();
    let mut client = crate::session::Client::with_mode_stack(client_id, metadata, mode_stack);
    // #474: Set compositor on per-client state (not shared)
    client.state.compositor = Some(Box::new(TestCompositor {
        focused: Some(WindowId::from_raw(1)),
    }));
    session.clients().add_client_with_state(client);

    // Build layout notification - this should hit the compositor branch
    let notification = build_layout_notification(&session, 12345, 1);
    assert_eq!(notification.event_type, "layout_changed");
    assert_eq!(notification.timestamp_ms, 12345);

    if let Some(notification::Payload::LayoutChanged(payload)) = notification.payload {
        // Compositor returned one placement
        assert_eq!(payload.windows.len(), 1);
        let win = &payload.windows[0];
        assert_eq!(win.window_id, 1);
        // buffer_id comes from active_buffer_for_client (domain driver, #753 E3)
        // No domain driver wired in this unit test -> None
        assert!(win.buffer_id.is_none());
        assert!(win.focused);
        // Verify rect from compositor (default terminal is 80x24)
        let rect = win.rect.as_ref().expect("rect should be present");
        assert_eq!(rect.x, 0);
        assert_eq!(rect.y, 0);
        assert_eq!(rect.width, 80);
        assert_eq!(rect.height, 24);
        // Focused window ID should be Some(1)
        assert_eq!(payload.focused_window_id, Some(1));
    } else {
        panic!("Expected LayoutChangedPayload");
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_build_layout_notification_compositor_no_client() {
    // #474: Compositor is now per-client. With no client registered,
    // there is no per-client state and no compositor, so the result is empty windows.
    let state = crate::session::SessionState::default();

    let session = Session::from_state(SessionId::new("compositor-no-client"), state);

    // No client registered - editing_state is None, so windows list is empty
    let notification = build_layout_notification(&session, 54321, 999);
    assert_eq!(notification.event_type, "layout_changed");

    if let Some(notification::Payload::LayoutChanged(payload)) = notification.payload {
        assert_eq!(payload.windows.len(), 0);
        assert!(payload.focused_window_id.is_none());
    } else {
        panic!("Expected LayoutChangedPayload");
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
#[allow(clippy::too_many_lines)]
fn test_build_layout_notification_compositor_with_active_buffer_fallback() {
    // Test compositor branch where client_windows has no matching window
    // but active_buffer is set as fallback
    use reovim_subsys_layout::{
        CompositeResult, Layer, LayerConfig, LayerId, Rect, RootCompositor, WindowId,
        WindowLayerCompositor, WindowPlacement, ZOrder, Zone,
    };

    struct TestCompositorTwoWindows;

    #[cfg_attr(coverage_nightly, coverage(off))]
    impl RootCompositor for TestCompositorTwoWindows {
        fn composite(&self, screen: Rect) -> CompositeResult {
            let p1 = WindowPlacement::new(
                WindowId::from_raw(10),
                LayerId::new(0),
                Zone::Tiled,
                Rect::new(0, 0, screen.width / 2, screen.height),
                ZOrder::new(100),
            );
            let p2 = WindowPlacement::new(
                WindowId::from_raw(20),
                LayerId::new(0),
                Zone::Tiled,
                Rect::new(screen.width / 2, 0, screen.width / 2, screen.height),
                ZOrder::new(101),
            );
            CompositeResult {
                placements: vec![p1, p2],
                focused: Some(WindowId::from_raw(10)),
                active_layer: Some(LayerId::new(0)),
                screen,
            }
        }

        fn create_layer(&mut self, _config: LayerConfig) -> LayerId {
            LayerId::new(0)
        }
        fn remove_layer(&mut self, _layer: LayerId) {}
        fn layer_by_label(&self, _label: &str) -> Option<LayerId> {
            None
        }
        fn layers(&self) -> Vec<&Layer> {
            Vec::new()
        }
        fn set_layer_visible(&mut self, _layer: LayerId, _visible: bool) {}
        fn set_layer_opacity(&mut self, _layer: LayerId, _opacity: f32) {}
        fn reorder_layer(&mut self, _layer: LayerId, _new_z: u16) {}
        fn set_active_layer(&mut self, _layer: LayerId) {}
        fn active_layer(&self) -> Option<LayerId> {
            Some(LayerId::new(0))
        }
        fn set_focus(&mut self, _window: WindowId) {}
        fn focused(&self) -> Option<WindowId> {
            Some(WindowId::from_raw(10))
        }
        fn focus_at(&mut self, _x: u16, _y: u16) -> Option<WindowId> {
            None
        }
        fn layer_compositor(&self, _layer: LayerId) -> Option<&dyn WindowLayerCompositor> {
            None
        }
        fn layer_compositor_mut(
            &mut self,
            _layer: LayerId,
        ) -> Option<&mut dyn WindowLayerCompositor> {
            None
        }
        fn window_count(&self) -> usize {
            2
        }
        fn set_screen(&mut self, _screen: Rect) {}
        fn layer_of(&self, _window: WindowId) -> Option<LayerId> {
            Some(LayerId::new(0))
        }
        fn boxed_clone(&self) -> Box<dyn RootCompositor> {
            Box::new(Self)
        }
        fn generation(&self) -> u64 {
            0
        }
        fn topology(&self) -> reovim_subsys_layout::LayoutTopology {
            reovim_subsys_layout::LayoutTopology::Single(WindowId::from_raw(10))
        }
    }

    // Create session with compositor that returns two windows
    let state = crate::session::SessionState::default();
    let session = Session::from_state(SessionId::new("compositor-fallback"), state);

    // Add a client with compositor; windows are domain-owned (#753 E3)
    let client_id = crate::session::ClientId::new(1);
    let mode = reovim_kernel::api::v1::ModeId::new(
        reovim_kernel::api::v1::ModuleId::new("test"),
        "normal",
    );
    let mode_stack = reovim_kernel::api::v1::ModeStack::new(mode);
    let metadata = crate::session::ClientMetadata::default();
    let mut client = crate::session::Client::with_mode_stack(client_id, metadata, mode_stack);
    // #474: Set compositor on per-client state (not shared)
    client.state.compositor = Some(Box::new(TestCompositorTwoWindows));
    // active_buffer is domain-owned (#753 E3) — not set here
    session.clients().add_client_with_state(client);

    let notification = build_layout_notification(&session, 99999, 1);

    if let Some(notification::Payload::LayoutChanged(payload)) = notification.payload {
        assert_eq!(payload.windows.len(), 2);

        // Both windows: buffer_id from active_buffer_for_client (domain driver, #753 E3)
        // No domain driver wired in this unit test -> None
        let win10 = payload.windows.iter().find(|w| w.window_id == 10).unwrap();
        assert!(win10.buffer_id.is_none());
        assert!(win10.focused); // focused_id is WindowId(10)

        let win20 = payload.windows.iter().find(|w| w.window_id == 20).unwrap();
        assert!(win20.buffer_id.is_none());
        assert!(!win20.focused);
    } else {
        panic!("Expected LayoutChangedPayload");
    }
}

// === Extension notification tests (#514) ===

#[test]
fn test_extension_changed_without_bridges_no_notification() {
    let mut changes = ChangeSet::new();
    changes.record_extension_change("cmdline".into());

    let session = Session::new(SessionId::new("test"));
    // No bridges provided - should not emit extension notifications
    let notifications = build_notifications(&changes, &session, 0, None);
    assert!(
        notifications.is_empty(),
        "Should not emit extension notifications without bridges"
    );
}

#[test]
fn test_extension_changed_with_bridges_unknown_kind() {
    use reovim_subsys_session::bridges::BridgeRegistry;

    let mut changes = ChangeSet::new();
    changes.record_extension_change("unknown".into());

    let session = Session::new(SessionId::new("test"));
    let registry = BridgeRegistry::new(); // empty - no bridges registered
    let notifications = build_notifications(&changes, &session, 0, Some(&registry));
    assert!(notifications.is_empty(), "Should not emit notification for unknown bridge kind");
}

/// Test extension for bridge tests, replacing module-cmdline dev-dependency.
struct TestBridgeExtension {
    active: bool,
}

impl reovim_subsys_session::SessionExtension for TestBridgeExtension {
    fn create() -> Self {
        Self { active: false }
    }
}

/// Test bridge that serializes `TestBridgeExtension` to JSON.
struct TestBridge;

impl reovim_subsys_session::bridges::ExtensionStateBridge for TestBridge {
    fn kind(&self) -> &'static str {
        "test-ext"
    }

    fn scope(&self) -> reovim_subsys_session::bridges::ExtensionScope {
        reovim_subsys_session::bridges::ExtensionScope::Client
    }

    fn snapshot(
        &self,
        extensions: &reovim_subsys_session::ExtensionMap,
    ) -> Option<serde_json::Value> {
        let ext = extensions.get::<TestBridgeExtension>()?;
        Some(serde_json::json!({
            "active": ext.active,
        }))
    }

    fn is_active(&self, extensions: &reovim_subsys_session::ExtensionMap) -> bool {
        extensions
            .get::<TestBridgeExtension>()
            .is_some_and(|e| e.active)
    }

    fn on_mode_changed(
        &self,
        _from: &str,
        _to: &str,
        _extensions: &mut reovim_subsys_session::ExtensionMap,
    ) {
    }
}

#[test]
fn test_extension_changed_with_bridge() {
    use reovim_subsys_session::bridges::BridgeRegistry;

    let mut changes = ChangeSet::new();
    changes.record_extension_change("test-ext".into());

    let session = Session::new(SessionId::new("test"));
    let client_id = ClientId::new(42);
    session.add_client(client_id);
    session.clients().update_client_state(client_id, |state| {
        let ext = state.extensions.get_or_insert::<TestBridgeExtension>();
        ext.active = true;
    });

    let mut registry = BridgeRegistry::new();
    registry.register(TestBridge);

    let notifications = build_notifications(&changes, &session, 42, Some(&registry));
    assert_eq!(notifications.len(), 1);
    assert_eq!(notifications[0].event_type, "extension_updated");
    if let Some(notification::Payload::ExtensionUpdated(payload)) = &notifications[0].payload {
        assert_eq!(payload.kind, "test-ext");
        assert_eq!(payload.client_id, 42);
        let data: serde_json::Value = serde_json::from_str(&payload.data).expect("valid JSON");
        assert_eq!(data["active"], true);
    } else {
        panic!("Expected ExtensionUpdated payload");
    }
}

#[test]
fn test_extension_not_changed_no_notification() {
    use reovim_subsys_session::bridges::BridgeRegistry;

    let changes = ChangeSet::new(); // No extension changes

    let session = Session::new(SessionId::new("test"));
    let mut registry = BridgeRegistry::new();
    registry.register(TestBridge);

    let notifications = build_notifications(&changes, &session, 0, Some(&registry));
    assert!(notifications.is_empty());
}

#[test]
fn test_build_extension_notification_shared_scope_returns_notification() {
    use reovim_subsys_session::{
        ExtensionMap,
        bridges::{BridgeRegistry, ExtensionScope, ExtensionStateBridge},
    };

    struct SharedBridge;
    #[cfg_attr(coverage_nightly, coverage(off))]
    impl ExtensionStateBridge for SharedBridge {
        fn kind(&self) -> &'static str {
            "shared-test"
        }
        fn scope(&self) -> ExtensionScope {
            ExtensionScope::Shared
        }
        fn snapshot(&self, _: &ExtensionMap) -> Option<serde_json::Value> {
            Some(serde_json::json!({"shared": true}))
        }
        fn is_active(&self, _: &ExtensionMap) -> bool {
            true
        }
    }

    let mut bridges = BridgeRegistry::new();
    bridges.register(SharedBridge);

    let session = Session::new(SessionId::new("shared-test"));
    let result = build_extension_notification("shared-test", &session, 12345, 1, &bridges);
    // Shared scope now produces a notification (#543)
    assert!(result.is_some());
    let notif = result.unwrap();
    assert_eq!(notif.event_type, "extension_updated");
    match &notif.payload {
        Some(notification::Payload::ExtensionUpdated(payload)) => {
            assert_eq!(payload.kind, "shared-test");
            assert!(payload.data.contains("shared"));
        }
        _ => panic!("expected ExtensionUpdated payload"),
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_build_extension_notification_shared_scope_empty_returns_none() {
    use reovim_subsys_session::{
        ExtensionMap,
        bridges::{BridgeRegistry, ExtensionScope, ExtensionStateBridge},
    };

    /// Bridge that returns None when no extension is present.
    struct EmptySharedBridge;
    #[cfg_attr(coverage_nightly, coverage(off))]
    impl ExtensionStateBridge for EmptySharedBridge {
        fn kind(&self) -> &'static str {
            "empty-shared"
        }
        fn scope(&self) -> ExtensionScope {
            ExtensionScope::Shared
        }
        fn snapshot(&self, _: &ExtensionMap) -> Option<serde_json::Value> {
            None
        }
        fn is_active(&self, _: &ExtensionMap) -> bool {
            false
        }
    }

    let mut bridges = BridgeRegistry::new();
    bridges.register(EmptySharedBridge);

    let session = Session::new(SessionId::new("empty-shared"));
    let result = build_extension_notification("empty-shared", &session, 12345, 1, &bridges);
    // snapshot() returns None → build returns None
    assert!(result.is_none());
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_build_notifications_presence_changed() {
    use {
        crate::session::{ClientId, ClientPresence, SyncMode},
        std::time::SystemTime,
    };

    let session = Session::new(SessionId::new("presence-test"));
    let client_id = ClientId::new(42);
    let presence = ClientPresence {
        client_id,
        client_type: "tui".to_string(),
        display_name: "test".to_string(),
        buffer_id: Some(1),
        sync_mode: SyncMode::Independent,
        joined_at: SystemTime::now(),
    };
    session.presence().join(presence);

    let mut changes = ChangeSet::new();
    changes.record_presence_change(client_id.as_usize());

    let notifications = build_notifications(&changes, &session, 12345, None);
    assert_eq!(notifications.len(), 1);
    assert_eq!(notifications[0].event_type, "presence_updated");
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_build_notifications_presence_changed_missing_client() {
    let session = Session::new(SessionId::new("presence-test2"));

    let mut changes = ChangeSet::new();
    changes.record_presence_change(999);

    let notifications = build_notifications(&changes, &session, 12345, None);
    assert!(notifications.is_empty());
}

// =========================================================================
// Projection notification tests (#753 B5)
// =========================================================================

mod projection_tests {
    use {
        super::*,
        crate::session::projection_store::{StoreUpdateResult, VersionedProjection},
        reovim_subsys_coordination::{DomainId, Projection, ProjectionDelivery, ProjectionTag},
    };

    fn make_persistent(tag: &str, payload: &[u8]) -> Projection {
        Projection {
            tag: ProjectionTag::from(tag),
            domain_id: DomainId(1),
            window_id: None,
            payload: payload.to_vec(),
            display: None,
            delivery: ProjectionDelivery::Persistent,
        }
    }

    fn make_transient(tag: &str, payload: &[u8]) -> Projection {
        Projection {
            tag: ProjectionTag::from(tag),
            domain_id: DomainId(1),
            window_id: None,
            payload: payload.to_vec(),
            display: None,
            delivery: ProjectionDelivery::Transient,
        }
    }

    #[test]
    fn empty_result_produces_no_notifications() {
        let result = StoreUpdateResult {
            changed: vec![],
            transient: vec![],
        };
        let notifications = build_projection_notifications(&result, 1, 99999);
        assert!(notifications.is_empty());
    }

    #[test]
    fn persistent_changed_produces_notification() {
        let result = StoreUpdateResult {
            changed: vec![VersionedProjection {
                projection: make_persistent("text.mode", b"NORMAL"),
                version: 42,
            }],
            transient: vec![],
        };
        let notifications = build_projection_notifications(&result, 7, 12345);

        assert_eq!(notifications.len(), 1);
        assert_eq!(notifications[0].event_type, "projection_updated");
        assert_eq!(notifications[0].timestamp_ms, 12345);

        if let Some(notification::Payload::ProjectionUpdated(ref p)) = notifications[0].payload {
            assert_eq!(p.tag, "text.mode");
            assert_eq!(p.domain_id, 1);
            assert_eq!(p.window_id, None); // None = session-wide
            assert_eq!(p.datum.as_ref().unwrap().content, b"NORMAL");
            assert!(!p.transient);
            assert_eq!(p.version, 42);
            assert_eq!(p.client_id, 7);
        } else {
            panic!("Expected ProjectionUpdatedPayload");
        }
    }

    #[test]
    fn transient_produces_notification_with_transient_flag() {
        let result = StoreUpdateResult {
            changed: vec![],
            transient: vec![make_transient("platform.haptic", &[0xFF])],
        };
        let notifications = build_projection_notifications(&result, 3, 55555);

        assert_eq!(notifications.len(), 1);

        if let Some(notification::Payload::ProjectionUpdated(ref p)) = notifications[0].payload {
            assert_eq!(p.tag, "platform.haptic");
            assert!(p.transient);
            assert_eq!(p.version, 0);
            assert_eq!(p.client_id, 3);
        } else {
            panic!("Expected ProjectionUpdatedPayload");
        }
    }

    #[test]
    fn mixed_changed_and_transient() {
        let result = StoreUpdateResult {
            changed: vec![VersionedProjection {
                projection: make_persistent("text.mode", b"INSERT"),
                version: 10,
            }],
            transient: vec![make_transient("platform.bell", &[0x01])],
        };
        let notifications = build_projection_notifications(&result, 1, 77777);

        assert_eq!(notifications.len(), 2);

        // First: persistent
        if let Some(notification::Payload::ProjectionUpdated(ref p)) = notifications[0].payload {
            assert!(!p.transient);
            assert_eq!(p.version, 10);
        } else {
            panic!("Expected persistent ProjectionUpdatedPayload");
        }

        // Second: transient
        if let Some(notification::Payload::ProjectionUpdated(ref p)) = notifications[1].payload {
            assert!(p.transient);
            assert_eq!(p.version, 0);
        } else {
            panic!("Expected transient ProjectionUpdatedPayload");
        }
    }

    #[test]
    fn display_field_propagated() {
        let mut proj = make_persistent("text.mode", b"VISUAL");
        proj.display = Some("VISUAL".to_string());

        let result = StoreUpdateResult {
            changed: vec![VersionedProjection {
                projection: proj,
                version: 5,
            }],
            transient: vec![],
        };
        let notifications = build_projection_notifications(&result, 1, 10000);

        if let Some(notification::Payload::ProjectionUpdated(ref p)) = notifications[0].payload {
            assert_eq!(p.datum.as_ref().unwrap().display, Some("VISUAL".to_string()));
        } else {
            panic!("Expected ProjectionUpdatedPayload");
        }
    }

    #[test]
    fn window_scoped_projection() {
        let mut proj = make_persistent("text.cursor", &[0, 0, 0, 5]);
        proj.window_id = Some(reovim_kernel::api::v1::WindowId::from_raw(42));

        let result = StoreUpdateResult {
            changed: vec![VersionedProjection {
                projection: proj,
                version: 1,
            }],
            transient: vec![],
        };
        let notifications = build_projection_notifications(&result, 1, 20000);

        if let Some(notification::Payload::ProjectionUpdated(ref p)) = notifications[0].payload {
            assert_eq!(p.window_id, Some(42));
        } else {
            panic!("Expected ProjectionUpdatedPayload");
        }
    }
}
