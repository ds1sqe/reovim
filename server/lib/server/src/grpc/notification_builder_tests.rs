use {super::*, crate::session::SessionId};

#[test]
fn test_current_timestamp_ms() {
    let ts = current_timestamp_ms();
    // Should be a reasonable recent timestamp (after 2024)
    assert!(ts > 1_700_000_000_000);
}

#[test]
fn test_build_notifications_empty_changes() {
    let changes = StateChanges::new();
    let session = Session::new(SessionId::new("test"));
    // Use client_id 0 - no client registered, so per-client lookups return None
    // and fallback to shared state (or return empty notifications)
    let notifications = build_notifications(&changes, &session, 0, None);
    assert!(notifications.is_empty());
}

#[test]
fn test_build_notifications_mode_changed() {
    let mut changes = StateChanges::new();
    changes.record_mode_change();

    let session = Session::new(SessionId::new("test"));
    // Use client_id 0 - fallback to shared state for mode
    let notifications = build_notifications(&changes, &session, 0, None);

    assert_eq!(notifications.len(), 1);
    assert_eq!(notifications[0].event_type, "mode_changed");
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_mode_notification_fallback_uses_shared_state() {
    // Phase #486: When client_id is not found, should fallback to shared state
    let session = Session::new(SessionId::new("fallback-test"));

    // Build mode notification for non-existent client
    let notification = build_mode_notification(&session, 12345, 999);

    // Should still produce a valid notification using shared state
    assert_eq!(notification.event_type, "mode_changed");
    if let Some(notification::Payload::ModeChanged(payload)) = notification.payload {
        // Default shared state mode is "normal"
        assert_eq!(payload.name, "normal");
        assert_eq!(payload.client_id, 999);
    } else {
        panic!("Expected ModeChangedPayload");
    }
}

#[test]
fn test_cursor_notification_returns_none_for_unknown_client() {
    // Phase #486: When client_id is not found, cursor notification returns None
    let session = Session::new(SessionId::new("cursor-test"));
    let buffer_id = reovim_kernel::api::v1::BufferId::from_raw(1);

    // Non-existent client should return None (no cursor to report)
    let notification = build_cursor_notification(&session, buffer_id, 12345, 999);
    assert!(notification.is_none());
}

#[test]
fn test_selection_notification_returns_none_for_unknown_client() {
    // Phase #486: When client_id is not found, selection notification returns None
    let session = Session::new(SessionId::new("selection-test"));
    let buffer_id = reovim_kernel::api::v1::BufferId::from_raw(1);

    // Non-existent client should return None (no selection to report)
    let notification = build_selection_notification(&session, buffer_id, 12345, 999);
    assert!(notification.is_none());
}

#[test]
fn test_viewport_notification_returns_none_for_unknown_client() {
    // Phase #486: When client_id is not found, viewport notification returns None
    let session = Session::new(SessionId::new("viewport-test"));
    let window_id = reovim_kernel::api::v1::WindowId::from_raw(1);

    // Non-existent client should return None (no viewport to report)
    let notification = build_viewport_notification(&session, window_id, 12345, 999);
    assert!(notification.is_none());
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_build_buffer_modified_notification() {
    let buffer_id = reovim_kernel::api::v1::BufferId::from_raw(42);
    let notification = build_buffer_modified_notification(buffer_id, 99999);

    assert_eq!(notification.event_type, "buffer_modified");
    assert_eq!(notification.timestamp_ms, 99999);

    if let Some(notification::Payload::BufferModified(payload)) = notification.payload {
        assert_eq!(payload.buffer_id, 42);
        assert!(payload.change.is_none());
    } else {
        panic!("Expected BufferModifiedPayload");
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_build_buffer_list_notification_added() {
    let buffer_id = reovim_kernel::api::v1::BufferId::from_raw(7);
    let notification = build_buffer_list_notification("added", buffer_id, 55555);

    assert_eq!(notification.event_type, "buffer_list_changed");
    assert_eq!(notification.timestamp_ms, 55555);

    if let Some(notification::Payload::BufferListChanged(payload)) = notification.payload {
        assert_eq!(payload.action, "added");
        assert_eq!(payload.buffer_id, 7);
    } else {
        panic!("Expected BufferListChangedPayload");
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_build_buffer_list_notification_removed() {
    let buffer_id = reovim_kernel::api::v1::BufferId::from_raw(3);
    let notification = build_buffer_list_notification("removed", buffer_id, 11111);

    if let Some(notification::Payload::BufferListChanged(payload)) = notification.payload {
        assert_eq!(payload.action, "removed");
        assert_eq!(payload.buffer_id, 3);
    } else {
        panic!("Expected BufferListChangedPayload");
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_build_option_notification_bool() {
    let opt_change = reovim_driver_session::api::OptionChange {
        name: "number".to_string(),
        value: reovim_kernel::api::v1::OptionValue::Bool(true),
        window_id: None,
    };
    let notification = build_option_notification(&opt_change, 77777);

    assert_eq!(notification.event_type, "option_changed");
    assert_eq!(notification.timestamp_ms, 77777);

    if let Some(notification::Payload::OptionChanged(payload)) = notification.payload {
        assert_eq!(payload.name, "number");
        assert_eq!(
            payload.value,
            Some(reovim_protocol::v2::option_changed_payload::Value::BoolValue(true))
        );
    } else {
        panic!("Expected OptionChangedPayload");
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_build_option_notification_integer() {
    let opt_change = reovim_driver_session::api::OptionChange {
        name: "tabstop".to_string(),
        value: reovim_kernel::api::v1::OptionValue::Integer(4),
        window_id: None,
    };
    let notification = build_option_notification(&opt_change, 88888);

    if let Some(notification::Payload::OptionChanged(payload)) = notification.payload {
        assert_eq!(payload.name, "tabstop");
        assert_eq!(
            payload.value,
            Some(reovim_protocol::v2::option_changed_payload::Value::IntValue(4))
        );
    } else {
        panic!("Expected OptionChangedPayload");
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_build_option_notification_string() {
    let opt_change = reovim_driver_session::api::OptionChange {
        name: "theme".to_string(),
        value: reovim_kernel::api::v1::OptionValue::String("monokai".to_string()),
        window_id: None,
    };
    let notification = build_option_notification(&opt_change, 66666);

    if let Some(notification::Payload::OptionChanged(payload)) = notification.payload {
        assert_eq!(payload.name, "theme");
        assert_eq!(
            payload.value,
            Some(reovim_protocol::v2::option_changed_payload::Value::StringValue(
                "monokai".to_string()
            ))
        );
    } else {
        panic!("Expected OptionChangedPayload");
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_build_option_notification_choice() {
    let opt_change = reovim_driver_session::api::OptionChange {
        name: "virtualedit".to_string(),
        value: reovim_kernel::api::v1::OptionValue::Choice {
            value: "block".to_string(),
            choices: vec!["block".to_string(), "all".to_string()],
        },
        window_id: None,
    };
    let notification = build_option_notification(&opt_change, 44444);

    if let Some(notification::Payload::OptionChanged(payload)) = notification.payload {
        assert_eq!(payload.name, "virtualedit");
        // Choice sends the value as a StringValue
        assert_eq!(
            payload.value,
            Some(reovim_protocol::v2::option_changed_payload::Value::StringValue(
                "block".to_string()
            ))
        );
    } else {
        panic!("Expected OptionChangedPayload");
    }
}

#[test]
fn test_build_notifications_buffer_modified() {
    let buffer_id = reovim_kernel::api::v1::BufferId::from_raw(5);
    let mut changes = StateChanges::new();
    changes.record_buffer_modified(buffer_id);

    let session = Session::new(SessionId::new("buf-mod-test"));
    let notifications = build_notifications(&changes, &session, 0, None);

    assert_eq!(notifications.len(), 1);
    assert_eq!(notifications[0].event_type, "buffer_modified");
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_build_notifications_buffer_created() {
    let buffer_id = reovim_kernel::api::v1::BufferId::from_raw(10);
    let mut changes = StateChanges::new();
    changes.buffers_created.push(buffer_id);

    let session = Session::new(SessionId::new("buf-create-test"));
    let notifications = build_notifications(&changes, &session, 0, None);

    assert_eq!(notifications.len(), 1);
    assert_eq!(notifications[0].event_type, "buffer_list_changed");

    if let Some(notification::Payload::BufferListChanged(payload)) = &notifications[0].payload {
        assert_eq!(payload.action, "added");
        assert_eq!(payload.buffer_id, 10);
    } else {
        panic!("Expected BufferListChangedPayload");
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_build_notifications_buffer_deleted() {
    let buffer_id = reovim_kernel::api::v1::BufferId::from_raw(20);
    let mut changes = StateChanges::new();
    changes.buffers_deleted.push(buffer_id);

    let session = Session::new(SessionId::new("buf-delete-test"));
    let notifications = build_notifications(&changes, &session, 0, None);

    assert_eq!(notifications.len(), 1);
    assert_eq!(notifications[0].event_type, "buffer_list_changed");

    if let Some(notification::Payload::BufferListChanged(payload)) = &notifications[0].payload {
        assert_eq!(payload.action, "removed");
        assert_eq!(payload.buffer_id, 20);
    } else {
        panic!("Expected BufferListChangedPayload");
    }
}

#[test]
fn test_build_notifications_option_changed() {
    let mut changes = StateChanges::new();
    changes
        .options_changed
        .push(reovim_driver_session::api::OptionChange {
            name: "number".to_string(),
            value: reovim_kernel::api::v1::OptionValue::Bool(true),
            window_id: None,
        });

    let session = Session::new(SessionId::new("opt-test"));
    let notifications = build_notifications(&changes, &session, 0, None);

    assert_eq!(notifications.len(), 1);
    assert_eq!(notifications[0].event_type, "option_changed");
}

#[test]
fn test_build_notifications_window_changed() {
    let mut changes = StateChanges::new();
    changes.window_changed = true;

    let session = Session::new(SessionId::new("win-test"));
    let notifications = build_notifications(&changes, &session, 0, None);

    assert_eq!(notifications.len(), 1);
    assert_eq!(notifications[0].event_type, "layout_changed");
}

#[test]
fn test_build_notifications_focus_changed() {
    let mut changes = StateChanges::new();
    changes.focus_changed = true;

    let session = Session::new(SessionId::new("focus-test"));
    let notifications = build_notifications(&changes, &session, 0, None);

    assert_eq!(notifications.len(), 1);
    assert_eq!(notifications[0].event_type, "layout_changed");
}

#[test]
fn test_build_notifications_multiple_changes() {
    let buffer_id = reovim_kernel::api::v1::BufferId::from_raw(1);
    let mut changes = StateChanges::new();
    changes.record_mode_change();
    changes.record_buffer_modified(buffer_id);
    changes.buffers_created.push(buffer_id);
    changes
        .options_changed
        .push(reovim_driver_session::api::OptionChange {
            name: "wrap".to_string(),
            value: reovim_kernel::api::v1::OptionValue::Bool(false),
            window_id: None,
        });

    let session = Session::new(SessionId::new("multi-test"));
    let notifications = build_notifications(&changes, &session, 0, None);

    // Should have: mode_changed, buffer_modified, buffer_list_changed (added), option_changed
    assert_eq!(notifications.len(), 4);

    let event_types: Vec<&str> = notifications
        .iter()
        .map(|n| n.event_type.as_str())
        .collect();
    assert!(event_types.contains(&"mode_changed"));
    assert!(event_types.contains(&"buffer_modified"));
    assert!(event_types.contains(&"buffer_list_changed"));
    assert!(event_types.contains(&"option_changed"));
}

#[test]
fn test_build_notifications_cursor_moved_no_client() {
    let buffer_id = reovim_kernel::api::v1::BufferId::from_raw(1);
    let mut changes = StateChanges::new();
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
    let mut changes = StateChanges::new();
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
    let mut changes = StateChanges::new();
    changes.scroll_changed = true;
    changes.scrolled_windows.push(window_id);

    let session = Session::new(SessionId::new("scroll-test"));
    let notifications = build_notifications(&changes, &session, 0, None);

    // No viewport notification because client doesn't exist
    assert!(notifications.is_empty());
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_mode_notification_insert_mode_detection() {
    let session = Session::new(SessionId::new("insert-test"));
    // Since the default mode is "normal", it won't be insert
    let notification = build_mode_notification(&session, 12345, 999);

    if let Some(notification::Payload::ModeChanged(payload)) = notification.payload {
        assert!(!payload.is_insert);
        assert_eq!(payload.display, "NORMAL");
    } else {
        panic!("Expected ModeChangedPayload");
    }
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

    let mut changes = StateChanges::new();
    changes.buffers_created.push(buf1);
    changes.buffers_created.push(buf2);
    changes.buffers_deleted.push(buf3);

    let session = Session::new(SessionId::new("multi-buf-test"));
    let notifications = build_notifications(&changes, &session, 0, None);

    // 2 created + 1 deleted = 3 buffer_list_changed notifications
    assert_eq!(notifications.len(), 3);

    let mut added_count = 0;
    let mut removed_count = 0;
    for n in &notifications {
        if let Some(notification::Payload::BufferListChanged(payload)) = &n.payload {
            match payload.action.as_str() {
                "added" => added_count += 1,
                "removed" => removed_count += 1,
                _ => panic!("Unexpected action"),
            }
        }
    }
    assert_eq!(added_count, 2);
    assert_eq!(removed_count, 1);
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_build_cursor_notification_with_client_and_window() {
    use {
        reovim_driver_session::Window,
        reovim_kernel::api::v1::{ModeId, ModeStack, ModuleId},
    };

    let session = Session::new(SessionId::new("cursor-client-test"));
    let client_id = crate::session::ClientId::new(1);

    // Add client with a window that has a buffer
    let mode = ModeId::new(ModuleId::new("test"), "normal");
    let mode_stack = ModeStack::new(mode);
    let buffer_id = reovim_kernel::api::v1::BufferId::from_raw(42);
    let window = Window::with_buffer(buffer_id);
    let metadata = crate::session::ClientMetadata::default();
    let client =
        crate::session::Client::with_mode_stack_and_window(client_id, metadata, mode_stack, window);
    session.clients().add_client_with_state(client);

    // Build cursor notification for this client
    let notification = build_cursor_notification(&session, buffer_id, 99999, 1);
    assert!(notification.is_some());

    let n = notification.unwrap();
    assert_eq!(n.event_type, "cursor_moved");
    assert_eq!(n.timestamp_ms, 99999);
    if let Some(notification::Payload::CursorMoved(payload)) = n.payload {
        assert_eq!(payload.client_id, 1);
        assert!(payload.position.is_some());
        let pos = payload.position.unwrap();
        assert_eq!(pos.line, 0);
        assert_eq!(pos.column, 0);
    } else {
        panic!("Expected CursorMovedPayload");
    }
}

#[test]
fn test_build_cursor_notification_wrong_buffer_returns_none() {
    use {
        reovim_driver_session::Window,
        reovim_kernel::api::v1::{ModeId, ModeStack, ModuleId},
    };

    let session = Session::new(SessionId::new("cursor-wrong-buf"));
    let client_id = crate::session::ClientId::new(1);

    let mode = ModeId::new(ModuleId::new("test"), "normal");
    let mode_stack = ModeStack::new(mode);
    let buffer_id = reovim_kernel::api::v1::BufferId::from_raw(42);
    let window = Window::with_buffer(buffer_id);
    let metadata = crate::session::ClientMetadata::default();
    let client =
        crate::session::Client::with_mode_stack_and_window(client_id, metadata, mode_stack, window);
    session.clients().add_client_with_state(client);

    // Ask for a different buffer - should return None
    let other_buffer = reovim_kernel::api::v1::BufferId::from_raw(99);
    let notification = build_cursor_notification(&session, other_buffer, 99999, 1);
    assert!(notification.is_none());
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_build_selection_notification_with_character_selection() {
    use {
        reovim_driver_session::{CursorPosition, Window, api::Selection},
        reovim_kernel::api::v1::{ModeId, ModeStack, ModuleId},
        reovim_types_text::Position as KernelPosition,
    };

    let session = Session::new(SessionId::new("sel-char-test"));
    let client_id = crate::session::ClientId::new(5);

    let mode = ModeId::new(ModuleId::new("test"), "visual");
    let mode_stack = ModeStack::new(mode);
    let buffer_id = reovim_kernel::api::v1::BufferId::from_raw(10);
    let mut window = Window::with_buffer(buffer_id);
    window.cursor = CursorPosition { line: 0, column: 3 };
    window.selection =
        Some(Selection::character(KernelPosition::new(0, 0), KernelPosition::new(0, 5)));
    let metadata = crate::session::ClientMetadata::default();
    let client =
        crate::session::Client::with_mode_stack_and_window(client_id, metadata, mode_stack, window);
    session.clients().add_client_with_state(client);

    let notification = build_selection_notification(&session, buffer_id, 11111, 5);
    assert!(notification.is_some());

    let n = notification.unwrap();
    assert_eq!(n.event_type, "selection_changed");
    if let Some(notification::Payload::SelectionChanged(payload)) = n.payload {
        assert!(payload.has_selection);
        assert_eq!(payload.visual_mode, Some("char".to_string()));
        assert_eq!(payload.client_id, 5);
        let sel = payload.selection.unwrap();
        assert_eq!(sel.start.as_ref().unwrap().line, 0);
        assert_eq!(sel.start.as_ref().unwrap().column, 0);
        assert_eq!(sel.end.as_ref().unwrap().line, 0);
        assert_eq!(sel.end.as_ref().unwrap().column, 5);
    } else {
        panic!("Expected SelectionChangedPayload");
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_build_selection_notification_with_line_selection() {
    use {
        reovim_driver_session::{Window, api::Selection},
        reovim_kernel::api::v1::{ModeId, ModeStack, ModuleId},
        reovim_types_text::Position as KernelPosition,
    };

    let session = Session::new(SessionId::new("sel-line-test"));
    let client_id = crate::session::ClientId::new(3);

    let mode = ModeId::new(ModuleId::new("test"), "visual-line");
    let mode_stack = ModeStack::new(mode);
    let buffer_id = reovim_kernel::api::v1::BufferId::from_raw(20);
    let mut window = Window::with_buffer(buffer_id);
    window.selection = Some(Selection::line(KernelPosition::new(1, 0), KernelPosition::new(3, 0)));
    let metadata = crate::session::ClientMetadata::default();
    let client =
        crate::session::Client::with_mode_stack_and_window(client_id, metadata, mode_stack, window);
    session.clients().add_client_with_state(client);

    let notification = build_selection_notification(&session, buffer_id, 22222, 3);
    assert!(notification.is_some());

    let n = notification.unwrap();
    if let Some(notification::Payload::SelectionChanged(payload)) = n.payload {
        assert!(payload.has_selection);
        assert_eq!(payload.visual_mode, Some("line".to_string()));
    } else {
        panic!("Expected SelectionChangedPayload");
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_build_selection_notification_with_block_selection() {
    use {
        reovim_driver_session::{Window, api::Selection},
        reovim_kernel::api::v1::{ModeId, ModeStack, ModuleId},
        reovim_types_text::Position as KernelPosition,
    };

    let session = Session::new(SessionId::new("sel-block-test"));
    let client_id = crate::session::ClientId::new(7);

    let mode = ModeId::new(ModuleId::new("test"), "visual-block");
    let mode_stack = ModeStack::new(mode);
    let buffer_id = reovim_kernel::api::v1::BufferId::from_raw(30);
    let mut window = Window::with_buffer(buffer_id);
    window.selection = Some(Selection::block(KernelPosition::new(0, 1), KernelPosition::new(2, 4)));
    let metadata = crate::session::ClientMetadata::default();
    let client =
        crate::session::Client::with_mode_stack_and_window(client_id, metadata, mode_stack, window);
    session.clients().add_client_with_state(client);

    let notification = build_selection_notification(&session, buffer_id, 33333, 7);
    assert!(notification.is_some());

    let n = notification.unwrap();
    if let Some(notification::Payload::SelectionChanged(payload)) = n.payload {
        assert!(payload.has_selection);
        assert_eq!(payload.visual_mode, Some("block".to_string()));
    } else {
        panic!("Expected SelectionChangedPayload");
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_build_selection_notification_no_selection() {
    use {
        reovim_driver_session::Window,
        reovim_kernel::api::v1::{ModeId, ModeStack, ModuleId},
    };

    let session = Session::new(SessionId::new("sel-none-test"));
    let client_id = crate::session::ClientId::new(2);

    let mode = ModeId::new(ModuleId::new("test"), "normal");
    let mode_stack = ModeStack::new(mode);
    let buffer_id = reovim_kernel::api::v1::BufferId::from_raw(15);
    let window = Window::with_buffer(buffer_id);
    let metadata = crate::session::ClientMetadata::default();
    let client =
        crate::session::Client::with_mode_stack_and_window(client_id, metadata, mode_stack, window);
    session.clients().add_client_with_state(client);

    // Window has no selection
    let notification = build_selection_notification(&session, buffer_id, 44444, 2);
    assert!(notification.is_some());

    let n = notification.unwrap();
    if let Some(notification::Payload::SelectionChanged(payload)) = n.payload {
        assert!(!payload.has_selection);
        assert!(payload.selection.is_none());
        assert!(payload.visual_mode.is_none());
    } else {
        panic!("Expected SelectionChangedPayload");
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_build_viewport_notification_with_client_window() {
    use {
        reovim_driver_session::{Viewport, Window},
        reovim_kernel::api::v1::{ModeId, ModeStack, ModuleId},
    };

    let session = Session::new(SessionId::new("viewport-test"));
    let client_id = crate::session::ClientId::new(4);

    let mode = ModeId::new(ModuleId::new("test"), "normal");
    let mode_stack = ModeStack::new(mode);
    let buffer_id = reovim_kernel::api::v1::BufferId::from_raw(50);
    let mut window = Window::with_buffer(buffer_id);
    let window_id = window.id;
    let mut viewport = Viewport::new(80, 24);
    viewport.scroll_top = 10;
    viewport.scroll_left = 5;
    window.viewport = viewport;
    let metadata = crate::session::ClientMetadata::default();
    let client =
        crate::session::Client::with_mode_stack_and_window(client_id, metadata, mode_stack, window);
    session.clients().add_client_with_state(client);

    let notification = build_viewport_notification(&session, window_id, 55555, 4);
    assert!(notification.is_some());

    let n = notification.unwrap();
    assert_eq!(n.event_type, "viewport_updated");
    assert_eq!(n.timestamp_ms, 55555);
    if let Some(notification::Payload::ViewportUpdated(payload)) = n.payload {
        assert_eq!(payload.viewport_id, window_id.as_usize() as u64);
        assert_eq!(payload.top_line, Some(10));
        assert_eq!(payload.left_col, Some(5));
        assert_eq!(payload.cursor_line, Some(0));
        assert_eq!(payload.cursor_col, Some(0));
    } else {
        panic!("Expected ViewportUpdatedPayload");
    }
}

#[test]
fn test_build_viewport_notification_wrong_window_returns_none() {
    use {
        reovim_driver_session::Window,
        reovim_kernel::api::v1::{ModeId, ModeStack, ModuleId},
    };

    let session = Session::new(SessionId::new("viewport-wrong-win"));
    let client_id = crate::session::ClientId::new(6);

    let mode = ModeId::new(ModuleId::new("test"), "normal");
    let mode_stack = ModeStack::new(mode);
    let buffer_id = reovim_kernel::api::v1::BufferId::from_raw(60);
    let window = Window::with_buffer(buffer_id);
    let metadata = crate::session::ClientMetadata::default();
    let client =
        crate::session::Client::with_mode_stack_and_window(client_id, metadata, mode_stack, window);
    session.clients().add_client_with_state(client);

    // Ask for a window that doesn't exist in client's layout
    let wrong_window_id = reovim_kernel::api::v1::WindowId::from_raw(99999);
    let notification = build_viewport_notification(&session, wrong_window_id, 66666, 6);
    assert!(notification.is_none());
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_build_mode_notification_with_registered_client() {
    use reovim_kernel::api::v1::{ModeId, ModeStack, ModuleId};

    let session = Session::new(SessionId::new("mode-client-test"));
    let client_id = crate::session::ClientId::new(8);

    // Register client with "insert" mode
    let mode = ModeId::new(ModuleId::new("test"), "insert");
    let mode_stack = ModeStack::new(mode);
    let metadata = crate::session::ClientMetadata::default();
    let client = crate::session::Client::with_mode_stack(client_id, metadata, mode_stack);
    session.clients().add_client_with_state(client);

    let notification = build_mode_notification(&session, 77777, 8);
    assert_eq!(notification.event_type, "mode_changed");
    if let Some(notification::Payload::ModeChanged(payload)) = notification.payload {
        assert_eq!(payload.name, "insert");
        assert_eq!(payload.display, "INSERT");
        assert!(payload.is_insert);
        assert_eq!(payload.client_id, 8);
    } else {
        panic!("Expected ModeChangedPayload");
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_build_layout_notification_with_client_windows() {
    use {
        reovim_driver_session::Window,
        reovim_kernel::api::v1::{ModeId, ModeStack, ModuleId},
    };

    let session = Session::new(SessionId::new("layout-client-test"));
    let client_id = crate::session::ClientId::new(9);

    let mode = ModeId::new(ModuleId::new("test"), "normal");
    let mode_stack = ModeStack::new(mode);
    let buffer_id = reovim_kernel::api::v1::BufferId::from_raw(70);
    let window = Window::with_buffer(buffer_id);
    let metadata = crate::session::ClientMetadata::default();
    let client =
        crate::session::Client::with_mode_stack_and_window(client_id, metadata, mode_stack, window);
    session.clients().add_client_with_state(client);

    let notification = build_layout_notification(&session, 88888, 9);
    assert_eq!(notification.event_type, "layout_changed");
    if let Some(notification::Payload::LayoutChanged(payload)) = notification.payload {
        // No compositor, so fallback to per-client windows
        assert_eq!(payload.windows.len(), 1);
        let win = &payload.windows[0];
        assert_eq!(win.buffer_id, Some(70));
        // Default window rect values
        assert_eq!(win.rect.as_ref().unwrap().width, 80);
        assert_eq!(win.rect.as_ref().unwrap().height, 24);
    } else {
        panic!("Expected LayoutChangedPayload");
    }
}

#[test]
fn test_build_notifications_cursor_moved_with_client() {
    use {
        reovim_driver_session::Window,
        reovim_kernel::api::v1::{ModeId, ModeStack, ModuleId},
    };

    let session = Session::new(SessionId::new("cursor-notify-test"));
    let client_id = crate::session::ClientId::new(10);
    let buffer_id = reovim_kernel::api::v1::BufferId::from_raw(80);

    // Register client with a window
    let mode = ModeId::new(ModuleId::new("test"), "normal");
    let mode_stack = ModeStack::new(mode);
    let window = Window::with_buffer(buffer_id);
    let metadata = crate::session::ClientMetadata::default();
    let client =
        crate::session::Client::with_mode_stack_and_window(client_id, metadata, mode_stack, window);
    session.clients().add_client_with_state(client);

    // Build notifications with cursor_moved
    let mut changes = StateChanges::new();
    changes.record_cursor_move(buffer_id);
    let notifications = build_notifications(&changes, &session, 10, None);

    assert_eq!(notifications.len(), 1);
    assert_eq!(notifications[0].event_type, "cursor_moved");
}

#[test]
fn test_build_notifications_selection_changed_with_client() {
    use {
        reovim_driver_session::{Window, api::Selection},
        reovim_kernel::api::v1::{ModeId, ModeStack, ModuleId},
        reovim_types_text::Position as KernelPosition,
    };

    let session = Session::new(SessionId::new("sel-notify-test"));
    let client_id = crate::session::ClientId::new(11);
    let buffer_id = reovim_kernel::api::v1::BufferId::from_raw(90);

    let mode = ModeId::new(ModuleId::new("test"), "visual");
    let mode_stack = ModeStack::new(mode);
    let mut window = Window::with_buffer(buffer_id);
    window.selection =
        Some(Selection::character(KernelPosition::new(0, 0), KernelPosition::new(0, 3)));
    let metadata = crate::session::ClientMetadata::default();
    let client =
        crate::session::Client::with_mode_stack_and_window(client_id, metadata, mode_stack, window);
    session.clients().add_client_with_state(client);

    let mut changes = StateChanges::new();
    changes.selection_changed = true;
    changes.affected_buffers.push(buffer_id);
    let notifications = build_notifications(&changes, &session, 11, None);

    assert_eq!(notifications.len(), 1);
    assert_eq!(notifications[0].event_type, "selection_changed");
}

#[test]
fn test_build_notifications_scroll_changed_with_client() {
    use {
        reovim_driver_session::Window,
        reovim_kernel::api::v1::{ModeId, ModeStack, ModuleId},
    };

    let session = Session::new(SessionId::new("scroll-notify-test"));
    let client_id = crate::session::ClientId::new(12);
    let buffer_id = reovim_kernel::api::v1::BufferId::from_raw(100);

    let mode = ModeId::new(ModuleId::new("test"), "normal");
    let mode_stack = ModeStack::new(mode);
    let window = Window::with_buffer(buffer_id);
    let window_id = window.id;
    let metadata = crate::session::ClientMetadata::default();
    let client =
        crate::session::Client::with_mode_stack_and_window(client_id, metadata, mode_stack, window);
    session.clients().add_client_with_state(client);

    let mut changes = StateChanges::new();
    changes.scroll_changed = true;
    changes.scrolled_windows.push(window_id);
    let notifications = build_notifications(&changes, &session, 12, None);

    assert_eq!(notifications.len(), 1);
    assert_eq!(notifications[0].event_type, "viewport_updated");
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
#[allow(clippy::too_many_lines)]
fn test_build_layout_notification_with_compositor() {
    use reovim_driver_layout::{
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
    }

    // Create a session state (compositor is now per-client, not shared)
    let state = crate::session::SessionState::default();

    let session = Session::from_state(SessionId::new("compositor-test"), state);

    // Add a client with a window matching the compositor's window_id
    let client_id = crate::session::ClientId::new(1);
    let mode = reovim_kernel::api::v1::ModeId::new(
        reovim_kernel::api::v1::ModuleId::new("test"),
        "normal",
    );
    let mode_stack = reovim_kernel::api::v1::ModeStack::new(mode);
    let buffer_id = reovim_kernel::api::v1::BufferId::from_raw(42);
    let mut window = reovim_driver_session::Window::with_buffer(buffer_id);
    // Override the window id to match the compositor placement
    window.id = reovim_kernel::api::v1::WindowId::from_raw(1);
    let metadata = crate::session::ClientMetadata::default();
    let mut client =
        crate::session::Client::with_mode_stack_and_window(client_id, metadata, mode_stack, window);
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
        assert_eq!(win.buffer_id, Some(42));
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
    use {
        reovim_driver_buffer::TestBufferManager,
        reovim_driver_layout::{
            CompositeResult, Layer, LayerConfig, LayerId, Rect, RootCompositor, WindowId,
            WindowLayerCompositor, WindowPlacement, ZOrder, Zone,
        },
        reovim_kernel::api::v1::{EventBus, KernelContext, OptionRegistry, ServiceRegistry},
        std::sync::Arc,
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
    }

    // Create a kernel with a real buffer manager so we can create a buffer
    let kernel = KernelContext::new(
        Arc::new(EventBus::new()),
        Arc::new(TestBufferManager::new()),
        Arc::new(OptionRegistry::new()),
        Arc::new(ServiceRegistry::new()),
    );

    let mut state = crate::session::SessionState::with_kernel(kernel);

    // Create a buffer so active_buffer is set
    let buffer_id = state.create_buffer("test content");

    let session = Session::from_state(SessionId::new("compositor-fallback"), state);

    // Add a client with a window for window_id=10 but NOT for window_id=20
    let client_id = crate::session::ClientId::new(1);
    let mode = reovim_kernel::api::v1::ModeId::new(
        reovim_kernel::api::v1::ModuleId::new("test"),
        "normal",
    );
    let mode_stack = reovim_kernel::api::v1::ModeStack::new(mode);
    let mut window = reovim_driver_session::Window::with_buffer(buffer_id);
    window.id = reovim_kernel::api::v1::WindowId::from_raw(10);
    let metadata = crate::session::ClientMetadata::default();
    let mut client =
        crate::session::Client::with_mode_stack_and_window(client_id, metadata, mode_stack, window);
    // #474: Set compositor on per-client state (not shared)
    client.state.compositor = Some(Box::new(TestCompositorTwoWindows));
    // Per-client active_buffer (#471): set so fallback for unmapped windows works
    client.state.active_buffer = Some(buffer_id);
    session.clients().add_client_with_state(client);

    let notification = build_layout_notification(&session, 99999, 1);

    if let Some(notification::Payload::LayoutChanged(payload)) = notification.payload {
        assert_eq!(payload.windows.len(), 2);

        // Window 10: client has a window for it -> gets buffer_id from client window
        let win10 = payload.windows.iter().find(|w| w.window_id == 10).unwrap();
        assert_eq!(win10.buffer_id, Some(buffer_id.as_usize() as u64));
        assert!(win10.focused); // focused_id is WindowId(10)

        // Window 20: client doesn't have this window, falls back to active_buffer
        let win20 = payload.windows.iter().find(|w| w.window_id == 20).unwrap();
        assert_eq!(win20.buffer_id, Some(buffer_id.as_usize() as u64));
        assert!(!win20.focused);
    } else {
        panic!("Expected LayoutChangedPayload");
    }
}

#[test]
fn test_build_notifications_multiple_option_changes() {
    let mut changes = StateChanges::new();
    changes
        .options_changed
        .push(reovim_driver_session::api::OptionChange {
            name: "number".to_string(),
            value: reovim_kernel::api::v1::OptionValue::Bool(true),
            window_id: None,
        });
    changes
        .options_changed
        .push(reovim_driver_session::api::OptionChange {
            name: "tabstop".to_string(),
            value: reovim_kernel::api::v1::OptionValue::Integer(8),
            window_id: None,
        });

    let session = Session::new(SessionId::new("multi-opt-test"));
    let notifications = build_notifications(&changes, &session, 0, None);

    assert_eq!(notifications.len(), 2);
    assert!(
        notifications
            .iter()
            .all(|n| n.event_type == "option_changed")
    );
}

// === Extension notification tests (#514) ===

#[test]
fn test_extension_changed_without_bridges_no_notification() {
    let mut changes = StateChanges::new();
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
    use reovim_driver_session::bridges::BridgeRegistry;

    let mut changes = StateChanges::new();
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

impl reovim_driver_session::SessionExtension for TestBridgeExtension {
    fn create() -> Self {
        Self { active: false }
    }
}

/// Test bridge that serializes `TestBridgeExtension` to JSON.
struct TestBridge;

impl reovim_driver_session::bridges::ExtensionStateBridge for TestBridge {
    fn kind(&self) -> &'static str {
        "test-ext"
    }

    fn scope(&self) -> reovim_driver_session::bridges::ExtensionScope {
        reovim_driver_session::bridges::ExtensionScope::Client
    }

    fn snapshot(
        &self,
        extensions: &reovim_driver_session::ExtensionMap,
    ) -> Option<serde_json::Value> {
        let ext = extensions.get::<TestBridgeExtension>()?;
        Some(serde_json::json!({
            "active": ext.active,
        }))
    }

    fn is_active(&self, extensions: &reovim_driver_session::ExtensionMap) -> bool {
        extensions
            .get::<TestBridgeExtension>()
            .is_some_and(|e| e.active)
    }

    fn on_mode_changed(
        &self,
        _from: &str,
        _to: &str,
        _extensions: &mut reovim_driver_session::ExtensionMap,
    ) {
    }
}

#[test]
fn test_extension_changed_with_bridge() {
    use reovim_driver_session::bridges::BridgeRegistry;

    let mut changes = StateChanges::new();
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
    use reovim_driver_session::bridges::BridgeRegistry;

    let changes = StateChanges::new(); // No extension changes

    let session = Session::new(SessionId::new("test"));
    let mut registry = BridgeRegistry::new();
    registry.register(TestBridge);

    let notifications = build_notifications(&changes, &session, 0, Some(&registry));
    assert!(notifications.is_empty());
}

#[test]
fn test_build_extension_notification_shared_scope_returns_notification() {
    use reovim_driver_session::{
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
    use reovim_driver_session::{
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
        cursor: (0, 0),
        visible_lines: (0, 24),
        mode: "NORMAL".to_string(),
        sync_mode: SyncMode::Independent,
        joined_at: SystemTime::now(),
    };
    session.presence().join(presence);

    let mut changes = StateChanges::new();
    changes.record_presence_change(client_id.as_usize());

    let notifications = build_notifications(&changes, &session, 12345, None);
    assert_eq!(notifications.len(), 1);
    assert_eq!(notifications[0].event_type, "presence_updated");
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_build_notifications_presence_changed_missing_client() {
    let session = Session::new(SessionId::new("presence-test2"));

    let mut changes = StateChanges::new();
    changes.record_presence_change(999);

    let notifications = build_notifications(&changes, &session, 12345, None);
    assert!(notifications.is_empty());
}
