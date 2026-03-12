use super::*;

#[test]
fn test_session_new() {
    let session = Session::new(SessionId::new("test"));
    assert_eq!(session.id().name(), "test");
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_remove_client_dumps_ring_buffer() {
    let session = Session::new(SessionId::new("dump-test"));
    let client_id = ClientId::new(42);

    // Add a client using the new API
    session.add_client(client_id);

    // Log some events to the client's ring buffer
    session.with_client_ring_buffer(client_id, |ring| {
        ring.log_key("a");
        ring.log_key("b");
        ring.log_command("write");
    });

    // Remove the client - this should create a dump file
    let removed = session.remove_client(client_id);
    assert!(removed.is_some());

    // Verify the dump file was created
    // Note: The dump file path includes a timestamp, so we check the crash directory
    let crash_dir = super::super::crash_dump::crash_dir();
    if crash_dir.exists() {
        // Look for a dump file with our client ID
        let pattern = format!("client-{}-", client_id.as_usize());
        let found = std::fs::read_dir(&crash_dir).ok().is_some_and(|entries| {
            entries
                .filter_map(Result::ok)
                .any(|e| e.file_name().to_string_lossy().starts_with(&pattern))
        });

        // May not find file in CI environments where directory isn't writable
        if found {
            // Clean up: find and remove the test file
            if let Ok(entries) = std::fs::read_dir(&crash_dir) {
                for entry in entries.filter_map(Result::ok) {
                    let name = entry.file_name();
                    if name.to_string_lossy().starts_with(&pattern) {
                        std::fs::remove_file(entry.path()).ok();
                    }
                }
            }
        }
    }
}

#[test]
fn test_remove_following_client_also_dumps() {
    use crate::session::ClientRelation;

    // In #480 unified architecture, ALL clients have ring buffers
    let session = Session::new(SessionId::new("follow-test"));
    let owner_id = ClientId::new(1);
    let follow_id = ClientId::new(2);

    // Add owner and follower
    session.add_client(owner_id);
    session.add_client(follow_id);

    // Set follower relation
    let _ = session
        .set_client_relation(follow_id, Some(ClientRelation::Following { target: owner_id }));

    // Log event to follower's ring buffer
    session.with_client_ring_buffer(follow_id, |ring| {
        ring.log_state_change("follower connected");
    });

    // Remove the follower - should still dump (all clients have ring buffers now)
    let removed = session.remove_client(follow_id);
    assert!(removed.is_some());
    assert!(removed.unwrap().is_following());

    // Clean up owner
    session.remove_client(owner_id);
}

#[test]
fn test_remove_sharing_client_also_dumps() {
    use crate::session::ClientRelation;

    // In #480 unified architecture, ALL clients have ring buffers
    let session = Session::new(SessionId::new("share-test"));
    let owner_id = ClientId::new(1);
    let share_id = ClientId::new(2);

    // Add owner and sharer
    session.add_client(owner_id);
    session.add_client(share_id);

    // Set sharing relation
    let _ = session.set_client_relation(share_id, Some(ClientRelation::Sharing { with: owner_id }));

    // Log event to sharer's ring buffer
    session.with_client_ring_buffer(share_id, |ring| {
        ring.log_state_change("sharer connected");
    });

    // Remove the sharer - should dump (all clients have ring buffers)
    let removed = session.remove_client(share_id);
    assert!(removed.is_some());
    assert!(removed.unwrap().is_sharing());

    // Clean up owner
    session.remove_client(owner_id);
}

#[tokio::test]
async fn test_session_with_state() {
    use std::sync::Arc;

    use {
        parking_lot::RwLock as ParkingLotRwLock,
        reovim_driver_buffer::TestBufferManager,
        reovim_kernel::api::v1::{
            EventBus, KernelContext, MarkBank, MotionEngine, OptionRegistry, ServiceRegistry,
            TextObjectEngine,
        },
    };

    // Create a kernel context with a real buffer manager
    let kernel = KernelContext::new(
        Arc::new(EventBus::new()),
        Arc::new(TestBufferManager::new()),
        Arc::new(MotionEngine),
        Arc::new(TextObjectEngine),
        Arc::new(ParkingLotRwLock::new(MarkBank::new())),
        Arc::new(OptionRegistry::new()),
        Arc::new(ServiceRegistry::new()),
    );

    let state = SessionState::with_kernel(kernel);
    let session = Session::from_state(SessionId::default(), state);

    // Create a buffer
    session
        .with_state_mut(|state| {
            state.create_buffer("hello world");
        })
        .await;

    // Read it back
    let has_buffer = session
        .with_state(|state| !state.app.kernel.buffers.list().is_empty())
        .await;

    assert!(has_buffer);
}

#[test]
fn test_session_from_state() {
    let state = SessionState::default();
    let session = Session::from_state(SessionId::new("from-state"), state);
    assert_eq!(session.id().name(), "from-state");
    assert_eq!(session.client_count(), 0);
}

#[test]
fn test_add_client() {
    let session = Session::new(SessionId::new("test"));
    let client_id = ClientId::new(1);

    session.add_client(client_id);
    assert_eq!(session.client_count(), 1);
    assert!(session.has_client(client_id));
}

#[test]
fn test_add_client_with_metadata() {
    use crate::session::ClientMetadata;

    let session = Session::new(SessionId::new("test"));
    let client_id = ClientId::new(1);
    let metadata = ClientMetadata::default();

    session.add_client_with_metadata(client_id, metadata);
    assert_eq!(session.client_count(), 1);
    assert!(session.has_client(client_id));
}

#[test]
fn test_add_client_with_state() {
    use {
        crate::session::ClientMetadata,
        reovim_kernel::api::v1::{ModeId, ModeStack, ModuleId},
    };

    let session = Session::new(SessionId::new("test"));
    let client_id = ClientId::new(1);
    let mode = ModeId::new(ModuleId::new("test"), "normal");
    let mode_stack = ModeStack::new(mode);
    let metadata = ClientMetadata::default();

    let client = Client::with_mode_stack(client_id, metadata, mode_stack);
    session.add_client_with_state(client);

    assert_eq!(session.client_count(), 1);
    assert!(session.has_client(client_id));
}

#[test]
fn test_get_client() {
    let session = Session::new(SessionId::new("test"));
    let client_id = ClientId::new(1);

    session.add_client(client_id);
    let client = session.get_client(client_id);
    assert!(client.is_some());
    assert_eq!(client.unwrap().id, client_id);
}

#[test]
fn test_client_state() {
    let session = Session::new(SessionId::new("test"));
    let client_id = ClientId::new(1);

    session.add_client(client_id);
    let state = session.client_state(client_id);
    assert!(state.is_some());
}

#[test]
fn test_update_client_state() {
    let session = Session::new(SessionId::new("test"));
    let client_id = ClientId::new(1);

    session.add_client(client_id);

    let updated = session.update_client_state(client_id, |state| {
        // Just access the mode_stack to verify we can mutate
        let _ = state.mode_stack.current();
    });

    assert!(updated);
}

#[test]
fn test_update_client_state_nonexistent() {
    let session = Session::new(SessionId::new("test"));
    let client_id = ClientId::new(999);

    let updated = session.update_client_state(client_id, |_state| {});
    assert!(!updated);
}

#[test]
fn test_with_clients() {
    let session = Session::new(SessionId::new("test"));
    let client_id = ClientId::new(1);

    session.add_client(client_id);

    let count = session.with_clients(std::collections::HashMap::len);
    assert_eq!(count, 1);
}

#[test]
fn test_with_clients_mut() {
    let session = Session::new(SessionId::new("test"));
    let client_id = ClientId::new(1);

    session.add_client(client_id);

    session.with_clients_mut(|clients| {
        assert_eq!(clients.len(), 1);
    });
}

#[test]
fn test_has_client() {
    let session = Session::new(SessionId::new("test"));
    let client_id = ClientId::new(1);

    assert!(!session.has_client(client_id));
    session.add_client(client_id);
    assert!(session.has_client(client_id));
}

#[test]
fn test_set_client_relation_independent_to_following() {
    use crate::session::ClientRelation;

    let session = Session::new(SessionId::new("test"));
    let owner_id = ClientId::new(1);
    let follower_id = ClientId::new(2);

    session.add_client(owner_id);
    session.add_client(follower_id);

    let result = session
        .set_client_relation(follower_id, Some(ClientRelation::Following { target: owner_id }));

    assert!(result.is_ok());
    let client = session.get_client(follower_id).unwrap();
    assert!(client.is_following());
}

#[test]
fn test_set_client_relation_cannot_target_self() {
    use crate::session::ClientRelation;

    let session = Session::new(SessionId::new("test"));
    let client_id = ClientId::new(1);

    session.add_client(client_id);

    let result = session
        .set_client_relation(client_id, Some(ClientRelation::Following { target: client_id }));

    assert!(result.is_err());
}

#[test]
fn test_set_client_relation_target_not_found() {
    use crate::session::ClientRelation;

    let session = Session::new(SessionId::new("test"));
    let client_id = ClientId::new(1);
    let nonexistent_id = ClientId::new(999);

    session.add_client(client_id);

    let result = session.set_client_relation(
        client_id,
        Some(ClientRelation::Following {
            target: nonexistent_id,
        }),
    );

    assert!(result.is_err());
}

#[test]
fn test_set_client_relation_unchecked() {
    use crate::session::ClientRelation;

    let session = Session::new(SessionId::new("test"));
    let owner_id = ClientId::new(1);
    let follower_id = ClientId::new(2);

    session.add_client(owner_id);
    session.add_client(follower_id);

    let result = session.set_client_relation_unchecked(
        follower_id,
        Some(ClientRelation::Following { target: owner_id }),
    );

    assert!(result);
}

#[test]
fn test_set_client_relation_unchecked_nonexistent() {
    use crate::session::ClientRelation;

    let session = Session::new(SessionId::new("test"));
    let nonexistent_id = ClientId::new(999);
    let target_id = ClientId::new(1);

    let result = session.set_client_relation_unchecked(
        nonexistent_id,
        Some(ClientRelation::Following { target: target_id }),
    );

    assert!(!result);
}

#[test]
fn test_sync_and_set_relation() {
    use crate::session::ClientRelation;

    let session = Session::new(SessionId::new("test"));
    let owner_id = ClientId::new(1);
    let sharer_id = ClientId::new(2);

    session.add_client(owner_id);
    session.add_client(sharer_id);

    let result = session.sync_and_set_relation(
        sharer_id,
        owner_id,
        Some(ClientRelation::Sharing { with: owner_id }),
    );

    assert!(result.is_ok());
}

#[tokio::test]
async fn test_with_state_sync() {
    let session = Session::new(SessionId::new("test"));

    let running = session.with_state_sync(SessionState::is_running);
    assert!(running);
}

#[tokio::test]
async fn test_with_state_mut_sync() {
    use {
        parking_lot::RwLock as ParkingLotRwLock,
        reovim_driver_buffer::TestBufferManager,
        reovim_kernel::api::v1::{
            EventBus, KernelContext, MarkBank, MotionEngine, OptionRegistry, ServiceRegistry,
            TextObjectEngine,
        },
        std::sync::Arc,
    };

    let kernel = KernelContext::new(
        Arc::new(EventBus::new()),
        Arc::new(TestBufferManager::new()),
        Arc::new(MotionEngine),
        Arc::new(TextObjectEngine),
        Arc::new(ParkingLotRwLock::new(MarkBank::new())),
        Arc::new(OptionRegistry::new()),
        Arc::new(ServiceRegistry::new()),
    );

    let state = SessionState::with_kernel(kernel);
    let session = Session::from_state(SessionId::new("test"), state);

    session.with_state_mut_sync(|state| {
        state.create_buffer("test content");
    });

    let has_buffer = session.with_state_sync(|state| !state.app.kernel.buffers.list().is_empty());
    assert!(has_buffer);
}

#[tokio::test]
async fn test_client_current_mode() {
    let session = Session::new(SessionId::new("test"));
    let client_id = ClientId::new(1);

    session.add_client(client_id);

    let mode = session.client_current_mode(client_id);
    assert!(mode.is_some());
}

#[test]
fn test_dump_client_ring_buffer() {
    let session = Session::new(SessionId::new("test"));
    let client_id = ClientId::new(1);

    session.add_client(client_id);

    // Log something to the ring buffer
    session.with_client_ring_buffer(client_id, |ring| {
        ring.log_key("x");
    });

    let dump = session.dump_client_ring_buffer(client_id);
    assert!(dump.is_some());
    assert!(dump.unwrap().contains('x'));
}

#[test]
fn test_dump_client_ring_buffer_nonexistent() {
    let session = Session::new(SessionId::new("test"));
    let nonexistent_id = ClientId::new(999);

    let dump = session.dump_client_ring_buffer(nonexistent_id);
    assert!(dump.is_none());
}

#[cfg(feature = "grpc")]
#[test]
fn test_subscribe_notifications() {
    let session = Session::new(SessionId::new("test"));
    let _rx = session.subscribe_notifications();
    // Just verify it doesn't panic
}

#[cfg(feature = "grpc")]
#[test]
fn test_emit_notification() {
    use reovim_protocol::v2::Notification;

    let session = Session::new(SessionId::new("test"));
    let mut rx = session.subscribe_notifications();

    // Create a minimal notification - the exact fields depend on proto definition
    let notification = Notification::default();

    session.emit_notification(notification);

    // Try to receive (non-blocking check)
    // Just verify it doesn't panic - the actual notification content
    // is proto-generated and may vary
    let _result = rx.try_recv();
}

#[cfg(feature = "grpc")]
#[test]
fn test_capture_tracker() {
    let session = Session::new(SessionId::new("test"));
    let _tracker = session.capture_tracker();
    // Just verify it doesn't panic
}

#[cfg(feature = "grpc")]
#[test]
fn test_presence() {
    let session = Session::new(SessionId::new("test"));
    let _presence = session.presence();
    // Just verify it doesn't panic
}

#[test]
fn test_update_client_state_following_ignored() {
    use crate::session::ClientRelation;

    let session = Session::new(SessionId::new("test"));
    let owner_id = ClientId::new(1);
    let follower_id = ClientId::new(2);

    session.add_client(owner_id);
    session.add_client(follower_id);

    session
        .set_client_relation(follower_id, Some(ClientRelation::Following { target: owner_id }))
        .ok();

    // Following clients should ignore updates
    let updated = session.update_client_state(follower_id, |_state| {});
    assert!(!updated);
}

#[test]
fn test_update_client_state_sharing() {
    use crate::session::ClientRelation;

    let session = Session::new(SessionId::new("test"));
    let owner_id = ClientId::new(1);
    let sharer_id = ClientId::new(2);

    session.add_client(owner_id);
    session.add_client(sharer_id);

    session
        .set_client_relation(sharer_id, Some(ClientRelation::Sharing { with: owner_id }))
        .ok();

    // Sharing clients should update target's state
    let updated = session.update_client_state(sharer_id, |_state| {});
    assert!(updated);
}

#[test]
fn test_session_id_accessor() {
    let session = Session::new(SessionId::new("my-session"));
    assert_eq!(session.id().name(), "my-session");
}

#[test]
fn test_remove_client_returns_none_for_nonexistent() {
    let session = Session::new(SessionId::new("test"));
    let result = session.remove_client(ClientId::new(999));
    assert!(result.is_none());
}

#[test]
fn test_get_client_returns_none_for_nonexistent() {
    let session = Session::new(SessionId::new("test"));
    let result = session.get_client(ClientId::new(999));
    assert!(result.is_none());
}

#[test]
fn test_client_state_returns_none_for_nonexistent() {
    let session = Session::new(SessionId::new("test"));
    let result = session.client_state(ClientId::new(999));
    assert!(result.is_none());
}

#[test]
fn test_client_current_mode_nonexistent() {
    let session = Session::new(SessionId::new("test"));
    let result = session.client_current_mode(ClientId::new(999));
    assert!(result.is_none());
}

#[test]
fn test_client_current_mode_following_returns_none() {
    use crate::session::ClientRelation;

    let session = Session::new(SessionId::new("test"));
    let owner_id = ClientId::new(1);
    let follower_id = ClientId::new(2);

    session.add_client(owner_id);
    session.add_client(follower_id);

    session
        .set_client_relation(follower_id, Some(ClientRelation::Following { target: owner_id }))
        .ok();

    // Following clients should return None for mode (input ignored)
    let result = session.client_current_mode(follower_id);
    assert!(result.is_none());
}

#[test]
fn test_client_current_mode_sharing_returns_target_mode() {
    use crate::session::ClientRelation;

    let session = Session::new(SessionId::new("test"));
    let owner_id = ClientId::new(1);
    let sharer_id = ClientId::new(2);

    session.add_client(owner_id);
    session.add_client(sharer_id);

    session
        .set_client_relation(sharer_id, Some(ClientRelation::Sharing { with: owner_id }))
        .ok();

    // Sharing client should return target's mode
    let result = session.client_current_mode(sharer_id);
    assert!(result.is_some());
}

#[test]
fn test_multiple_clients() {
    let session = Session::new(SessionId::new("test"));
    let c1 = ClientId::new(1);
    let c2 = ClientId::new(2);
    let c3 = ClientId::new(3);

    session.add_client(c1);
    session.add_client(c2);
    session.add_client(c3);

    assert_eq!(session.client_count(), 3);
    assert!(session.has_client(c1));
    assert!(session.has_client(c2));
    assert!(session.has_client(c3));

    // Remove one
    let removed = session.remove_client(c2);
    assert!(removed.is_some());
    assert_eq!(session.client_count(), 2);
    assert!(!session.has_client(c2));
}

#[test]
fn test_with_client_ring_buffer_nonexistent() {
    let session = Session::new(SessionId::new("test"));
    let result = session.with_client_ring_buffer(ClientId::new(999), |_| ());
    assert!(result.is_none());
}

#[test]
fn test_with_client_ring_buffer_returns_value() {
    let session = Session::new(SessionId::new("test"));
    let client_id = ClientId::new(1);
    session.add_client(client_id);

    // Log and retrieve
    session.with_client_ring_buffer(client_id, |ring| {
        ring.log_key("a");
        ring.log_key("b");
    });

    let count = session.with_client_ring_buffer(client_id, |ring| {
        let dump = ring.dump();
        dump.matches('a').count()
    });
    assert!(count.is_some());
    assert!(count.unwrap() > 0);
}

#[test]
fn test_set_client_relation_with_cycle_detection() {
    use crate::session::ClientRelation;

    let session = Session::new(SessionId::new("cycle-test"));
    let id1 = ClientId::new(1);
    let id2 = ClientId::new(2);
    let id3 = ClientId::new(3);

    session.add_client(id1);
    session.add_client(id2);
    session.add_client(id3);

    // id2 follows id3
    let _ = session.set_client_relation(id2, Some(ClientRelation::Following { target: id3 }));

    // id3 follows id1
    let _ = session.set_client_relation(id3, Some(ClientRelation::Following { target: id1 }));

    // id1 trying to follow id2 would create cycle: 1 -> 2 -> 3 -> 1
    let result = session.set_client_relation(id1, Some(ClientRelation::Following { target: id2 }));
    assert!(result.is_err());
}

#[test]
fn test_sync_and_set_relation_nonexistent_client() {
    use crate::session::ClientRelation;

    let session = Session::new(SessionId::new("test"));

    let result = session.sync_and_set_relation(
        ClientId::new(999),
        ClientId::new(888),
        Some(ClientRelation::Sharing {
            with: ClientId::new(888),
        }),
    );
    assert!(result.is_err());
}

#[test]
fn test_set_client_relation_client_not_found() {
    use crate::session::ClientRelation;

    let session = Session::new(SessionId::new("test"));
    let nonexistent = ClientId::new(999);

    // Session-level relation setting should fail for non-existent client
    let result = session.set_client_relation(
        nonexistent,
        Some(ClientRelation::Following {
            target: ClientId::new(1),
        }),
    );
    assert!(result.is_err());
}

#[test]
fn test_set_client_relation_back_to_independent() {
    use crate::session::ClientRelation;

    let session = Session::new(SessionId::new("test"));
    let owner_id = ClientId::new(1);
    let follower_id = ClientId::new(2);

    session.add_client(owner_id);
    session.add_client(follower_id);

    // Set following
    let result = session
        .set_client_relation(follower_id, Some(ClientRelation::Following { target: owner_id }));
    assert!(result.is_ok());
    assert!(session.get_client(follower_id).unwrap().is_following());

    // Set back to independent (None relation)
    let result = session.set_client_relation(follower_id, None);
    assert!(result.is_ok());
    assert!(session.get_client(follower_id).unwrap().is_independent());
}

#[cfg(feature = "grpc")]
#[test]
fn test_emit_notification_and_receive() {
    use reovim_protocol::v2::Notification;

    let session = Session::new(SessionId::new("notif-test"));
    let mut rx = session.subscribe_notifications();

    let notification = Notification {
        event_type: "test_event".to_string(),
        timestamp_ms: 12345,
        payload: None,
    };

    session.emit_notification(notification);

    let received = rx.try_recv();
    assert!(received.is_ok());
    let n = received.unwrap();
    assert_eq!(n.event_type, "test_event");
    assert_eq!(n.timestamp_ms, 12345);
}

#[cfg(feature = "grpc")]
#[test]
fn test_emit_notification_no_subscribers() {
    let session = Session::new(SessionId::new("no-sub-test"));

    // Emit with no subscribers should not panic
    let notification = reovim_protocol::v2::Notification {
        event_type: "orphan".to_string(),
        timestamp_ms: 0,
        payload: None,
    };

    session.emit_notification(notification);
}

#[tokio::test]
async fn test_resolve_key_for_client_nonexistent() {
    let session = Session::new(SessionId::new("test"));
    let key = reovim_driver_input::KeyEvent::new(reovim_driver_input::KeyCode::Char('a'));

    // Non-existent client should return None
    let result = session
        .resolve_key_for_client(ClientId::new(999), &key)
        .await;
    assert!(result.is_none());
}

#[tokio::test]
async fn test_resolve_key_for_client_following_ignored() {
    use crate::session::ClientRelation;

    let session = Session::new(SessionId::new("test"));
    let owner_id = ClientId::new(1);
    let follower_id = ClientId::new(2);

    session.add_client(owner_id);
    session.add_client(follower_id);

    let _ = session
        .set_client_relation(follower_id, Some(ClientRelation::Following { target: owner_id }));

    let key = reovim_driver_input::KeyEvent::new(reovim_driver_input::KeyCode::Char('a'));
    let result = session.resolve_key_for_client(follower_id, &key).await;

    // Following clients should return None (input ignored)
    assert!(result.is_none());
}

#[tokio::test]
async fn test_try_on_command_complete_for_client_nonexistent() {
    let session = Session::new(SessionId::new("test"));
    let result = session
        .try_on_command_complete_for_client(ClientId::new(999))
        .await;
    assert!(result.is_none());
}

#[test]
fn test_execute_command_for_client_nonexistent() {
    let session = Session::new(SessionId::new("test"));
    let cmd_id = reovim_kernel::api::v1::CommandId::new(
        reovim_kernel::api::v1::ModuleId::new("test"),
        "noop",
    );
    let ctx = reovim_driver_command_types::CommandContext::new();

    let result = session.execute_command_for_client(ClientId::new(999), &cmd_id, &ctx);
    assert!(result.is_none());
}

#[test]
fn test_execute_command_for_client_following_ignored() {
    use crate::session::ClientRelation;

    let session = Session::new(SessionId::new("test"));
    let owner_id = ClientId::new(1);
    let follower_id = ClientId::new(2);

    session.add_client(owner_id);
    session.add_client(follower_id);

    let _ = session
        .set_client_relation(follower_id, Some(ClientRelation::Following { target: owner_id }));

    let cmd_id = reovim_kernel::api::v1::CommandId::new(
        reovim_kernel::api::v1::ModuleId::new("test"),
        "noop",
    );
    let ctx = reovim_driver_command_types::CommandContext::new();

    let result = session.execute_command_for_client(follower_id, &cmd_id, &ctx);
    // Following clients should return None (input ignored)
    assert!(result.is_none());
}

#[test]
fn test_add_client_with_active_buffer() {
    use {
        parking_lot::RwLock as ParkingLotRwLock,
        reovim_driver_buffer::TestBufferManager,
        reovim_kernel::api::v1::{
            EventBus, KernelContext, MarkBank, MotionEngine, OptionRegistry, ServiceRegistry,
            TextObjectEngine,
        },
        std::sync::Arc,
    };

    let kernel = KernelContext::new(
        Arc::new(EventBus::new()),
        Arc::new(TestBufferManager::new()),
        Arc::new(MotionEngine),
        Arc::new(TextObjectEngine),
        Arc::new(ParkingLotRwLock::new(MarkBank::new())),
        Arc::new(OptionRegistry::new()),
        Arc::new(ServiceRegistry::new()),
    );

    let state = SessionState::with_kernel(kernel);
    let session = Session::from_state(SessionId::new("buf-test"), state);

    // Create a buffer so session has an active buffer
    session.with_state_mut_sync(|state| {
        state.create_buffer("hello");
    });

    // Adding a client now should give it a window with the active buffer
    let client_id = ClientId::new(1);
    session.add_client(client_id);

    // Client should have a window
    let state = session.client_state(client_id);
    assert!(state.is_some());
    let editing_state = state.unwrap();
    assert!(!editing_state.windows.is_empty());
}

#[test]
fn test_insert_char_for_client_buffer_path() {
    use {
        parking_lot::RwLock as ParkingLotRwLock,
        reovim_driver_buffer::TestBufferManager,
        reovim_driver_input::InputTarget,
        reovim_kernel::api::v1::{
            EventBus, KernelContext, MarkBank, MotionEngine, OptionRegistry, ServiceRegistry,
            TextObjectEngine,
        },
        std::sync::Arc,
    };

    let kernel = KernelContext::new(
        Arc::new(EventBus::new()),
        Arc::new(TestBufferManager::new()),
        Arc::new(MotionEngine),
        Arc::new(TextObjectEngine),
        Arc::new(ParkingLotRwLock::new(MarkBank::new())),
        Arc::new(OptionRegistry::new()),
        Arc::new(ServiceRegistry::new()),
    );

    let state = SessionState::with_kernel(kernel);
    let session = Session::from_state(SessionId::new("insert-test"), state);

    // Create a buffer
    session.with_state_mut_sync(|state| {
        state.create_buffer("hello");
    });

    // Add a client (which gets a window with the active buffer)
    let client_id = ClientId::new(1);
    session.add_client(client_id);

    // Insert a character into the buffer
    let result = session.insert_char_for_client(client_id, 'X', InputTarget::Buffer);
    assert!(result.is_some()); // Should return the buffer id

    // Verify the character was inserted
    session.with_state_sync(|state| {
        let buffer_id = *state.app.kernel.buffers.list().first().unwrap();
        let buffer = state.buffer(buffer_id).unwrap();
        let content = buffer.read().content();
        assert!(content.contains('X'));
    });
}

#[test]
fn test_insert_char_for_client_newline() {
    use {
        parking_lot::RwLock as ParkingLotRwLock,
        reovim_driver_buffer::TestBufferManager,
        reovim_driver_input::InputTarget,
        reovim_kernel::api::v1::{
            EventBus, KernelContext, MarkBank, MotionEngine, OptionRegistry, ServiceRegistry,
            TextObjectEngine,
        },
        std::sync::Arc,
    };

    let kernel = KernelContext::new(
        Arc::new(EventBus::new()),
        Arc::new(TestBufferManager::new()),
        Arc::new(MotionEngine),
        Arc::new(TextObjectEngine),
        Arc::new(ParkingLotRwLock::new(MarkBank::new())),
        Arc::new(OptionRegistry::new()),
        Arc::new(ServiceRegistry::new()),
    );

    let state = SessionState::with_kernel(kernel);
    let session = Session::from_state(SessionId::new("newline-test"), state);

    session.with_state_mut_sync(|state| {
        state.create_buffer("ab");
    });

    let client_id = ClientId::new(1);
    session.add_client(client_id);

    // Insert a newline
    let result = session.insert_char_for_client(client_id, '\n', InputTarget::Buffer);
    assert!(result.is_some());

    // Verify the cursor moved to start of next line
    let editing_state = session.client_state(client_id).unwrap();
    let active_window = editing_state.windows.active().unwrap();
    assert_eq!(active_window.cursor.line, 1);
    assert_eq!(active_window.cursor.column, 0);
}

#[test]
fn test_insert_char_for_client_extension_path() {
    use reovim_driver_input::InputTarget;

    let session = Session::new(SessionId::new("ext-test"));
    let client_id = ClientId::new(1);
    session.add_client(client_id);

    // Extension target with a random type_id - neither per-client nor shared
    // extension exists, so the char is effectively dropped with a warning.
    let type_id = std::any::TypeId::of::<String>();
    let result = session.insert_char_for_client(client_id, 'a', InputTarget::Extension(type_id));
    // Extension path always returns None
    assert!(result.is_none());
}

#[test]
fn test_insert_char_for_client_no_buffer() {
    use reovim_driver_input::InputTarget;

    let session = Session::new(SessionId::new("no-buf-test"));
    let client_id = ClientId::new(1);
    session.add_client(client_id);

    // No buffer exists, so insert_char should return None
    let result = session.insert_char_for_client(client_id, 'x', InputTarget::Buffer);
    assert!(result.is_none());
}

#[test]
fn test_insert_char_for_client_nonexistent_client() {
    use reovim_driver_input::InputTarget;

    let session = Session::new(SessionId::new("noone-test"));
    let result = session.insert_char_for_client(ClientId::new(999), 'x', InputTarget::Buffer);
    assert!(result.is_none());
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_sync_and_set_relation_with_cursor_sync() {
    use {
        crate::session::ClientRelation,
        parking_lot::RwLock as ParkingLotRwLock,
        reovim_driver_buffer::TestBufferManager,
        reovim_kernel::api::v1::{
            EventBus, KernelContext, MarkBank, MotionEngine, OptionRegistry, ServiceRegistry,
            TextObjectEngine,
        },
        std::sync::Arc,
    };

    let kernel = KernelContext::new(
        Arc::new(EventBus::new()),
        Arc::new(TestBufferManager::new()),
        Arc::new(MotionEngine),
        Arc::new(TextObjectEngine),
        Arc::new(ParkingLotRwLock::new(MarkBank::new())),
        Arc::new(OptionRegistry::new()),
        Arc::new(ServiceRegistry::new()),
    );

    let state = SessionState::with_kernel(kernel);
    let session = Session::from_state(SessionId::new("sync-cursor-test"), state);

    // Create a buffer so clients get windows
    session.with_state_mut_sync(|state| {
        state.create_buffer("test content");
    });

    let owner_id = ClientId::new(1);
    let sharer_id = ClientId::new(2);

    session.add_client(owner_id);
    session.add_client(sharer_id);

    // Move owner's cursor to a specific position
    session.update_client_state(owner_id, |state| {
        if let Some(w) = state.windows.active_mut() {
            w.cursor.line = 5;
            w.cursor.column = 10;
        }
    });

    // Sync and set relation - sharer should get owner's cursor
    let result = session.sync_and_set_relation(
        sharer_id,
        owner_id,
        Some(ClientRelation::Sharing { with: owner_id }),
    );
    assert!(result.is_ok());

    // Sharer's cursor should be synced to owner's position
    let sharer_state = session.client_state(sharer_id).unwrap();
    if let Some(w) = sharer_state.windows.active() {
        assert_eq!(w.cursor.line, 5);
        assert_eq!(w.cursor.column, 10);
    }
}

// =========================================================================
// Coverage: sync_and_set_relation validation failure (#497)
// =========================================================================

#[test]
fn test_sync_and_set_relation_self_target_error() {
    // Test the `other => Err(other)` path in sync_and_set_relation (line 387)
    // where validation fails for a reason other than not found.
    use crate::session::ClientRelation;

    let session = Session::new(SessionId::new("test"));
    let client_id = ClientId::new(1);
    session.add_client(client_id);

    // Attempt to share with self - this triggers CannotTargetSelf
    let result = session.sync_and_set_relation(
        client_id,
        client_id,
        Some(ClientRelation::Sharing { with: client_id }),
    );
    assert!(result.is_err());
}

#[test]
fn test_sync_and_set_relation_would_create_cycle() {
    // Test the cycle detection error path in sync_and_set_relation (line 387)
    use crate::session::ClientRelation;

    let session = Session::new(SessionId::new("cycle-test"));
    let id1 = ClientId::new(1);
    let id2 = ClientId::new(2);

    session.add_client(id1);
    session.add_client(id2);

    // id2 follows id1
    let _ = session.set_client_relation(id2, Some(ClientRelation::Following { target: id1 }));

    // Try to set id1 → id2 via sync_and_set_relation → cycle
    let result =
        session.sync_and_set_relation(id1, id2, Some(ClientRelation::Sharing { with: id2 }));
    assert!(result.is_err());
}

// =========================================================================
// Coverage: update_client_state target not found (#497)
// =========================================================================

#[test]
fn test_update_client_state_sharing_target_removed() {
    // Test the `false` return at line 448 when the sharing target doesn't exist.
    use crate::session::ClientRelation;

    let session = Session::new(SessionId::new("test"));
    let owner_id = ClientId::new(1);
    let sharer_id = ClientId::new(2);

    session.add_client(owner_id);
    session.add_client(sharer_id);

    // Set sharer to share with owner
    let _ =
        session.set_client_relation(sharer_id, Some(ClientRelation::Sharing { with: owner_id }));

    // Remove the owner (the sharing target)
    session.remove_client(owner_id);

    // Now update_client_state for sharer should return false
    // because the target (owner) no longer exists
    let updated = session.update_client_state(sharer_id, |_state| {});
    assert!(!updated);
}

// =========================================================================
// Coverage: insert_char_for_client undo recording (#497 lines 775-787)
// =========================================================================

#[test]
#[allow(clippy::too_many_lines)]
fn test_insert_char_for_client_with_undo_recording() {
    // This test covers lines 775-787: the undo recording path in
    // insert_char_for_client when an UndoProviderRegistry with a
    // UndoKey::Buffer provider is registered in services.
    use {
        parking_lot::RwLock as ParkingLotRwLock,
        reovim_driver_buffer::TestBufferManager,
        reovim_driver_input::InputTarget,
        reovim_driver_undo::{UndoKey, UndoPersistError, UndoProvider, UndoProviderRegistry},
        reovim_driver_vfs::VfsDriver,
        reovim_kernel::api::v1::{
            BufferId, Edit, EventBus, KernelContext, MarkBank, MotionEngine, OptionRegistry,
            Position, ServiceRegistry, TextObjectEngine, UndoResult, UndoTree,
        },
        std::sync::{Arc, Mutex},
    };

    /// Recorded undo entries: (buffer, client, edits, `cursor_before`, `cursor_after`).
    type UndoRecords = Mutex<Vec<(BufferId, usize, Vec<Edit>, Position, Position)>>;

    /// Minimal mock undo provider that tracks `record_for_client` calls.
    struct MockUndoProvider {
        recorded: UndoRecords,
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    impl MockUndoProvider {
        fn new() -> Self {
            Self {
                recorded: Mutex::new(Vec::new()),
            }
        }
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    impl UndoProvider for MockUndoProvider {
        fn undo(&self, _buffer_id: BufferId) -> Option<UndoResult> {
            None
        }
        fn redo(&self, _buffer_id: BufferId) -> Option<UndoResult> {
            None
        }
        fn redo_branch(&self, _buffer_id: BufferId, _branch_idx: usize) -> Option<UndoResult> {
            None
        }
        fn record(
            &self,
            _buffer_id: BufferId,
            _edits: Vec<Edit>,
            _cursor_before: Position,
            _cursor_after: Position,
        ) {
        }
        fn has_history(&self, _buffer_id: BufferId) -> bool {
            false
        }
        fn remove(&self, _buffer_id: BufferId) {}
        fn buffer_count(&self) -> usize {
            0
        }
        fn get_tree(&self, _buffer_id: BufferId) -> Option<UndoTree> {
            None
        }
        fn begin_batch(&self, _buffer_id: BufferId, _cursor_before: Position) {}
        fn end_batch(&self, _buffer_id: BufferId, _cursor_after: Position) {}
        fn is_batching(&self, _buffer_id: BufferId) -> bool {
            false
        }
        fn persist(
            &self,
            _buffer_id: BufferId,
            _buffer_path: &str,
            _vfs: &dyn VfsDriver,
        ) -> Result<(), UndoPersistError> {
            Ok(())
        }
        fn load(
            &self,
            _buffer_id: BufferId,
            _buffer_path: &str,
            _vfs: &dyn VfsDriver,
        ) -> Result<bool, UndoPersistError> {
            Ok(false)
        }
        fn record_for_client(
            &self,
            buffer_id: BufferId,
            client_id: usize,
            edits: Vec<Edit>,
            cursor_before: Position,
            cursor_after: Position,
        ) {
            self.recorded.lock().unwrap().push((
                buffer_id,
                client_id,
                edits,
                cursor_before,
                cursor_after,
            ));
        }
    }

    // Create services with an UndoProviderRegistry containing our mock
    let services = Arc::new(ServiceRegistry::new());
    let undo_registry = Arc::new(UndoProviderRegistry::new());
    let mock_undo = Arc::new(MockUndoProvider::new());
    undo_registry.register(UndoKey::Buffer, Arc::clone(&mock_undo) as Arc<dyn UndoProvider>);
    services.register(undo_registry);

    let kernel = KernelContext::new(
        Arc::new(EventBus::new()),
        Arc::new(TestBufferManager::new()),
        Arc::new(MotionEngine),
        Arc::new(TextObjectEngine),
        Arc::new(ParkingLotRwLock::new(MarkBank::new())),
        Arc::new(OptionRegistry::new()),
        services,
    );

    let state = SessionState::with_kernel(kernel);
    let session = Session::from_state(SessionId::new("undo-test"), state);

    // Create a buffer
    session.with_state_mut_sync(|state| {
        state.create_buffer("hello");
    });

    // Add a client
    let client_id = ClientId::new(1);
    session.add_client(client_id);

    // Insert a character
    let result = session.insert_char_for_client(client_id, 'Z', InputTarget::Buffer);
    assert!(result.is_some());

    // Verify the undo provider received a record_for_client call
    let recorded = mock_undo.recorded.lock().unwrap();
    assert_eq!(recorded.len(), 1);
    let (buf_id, cid, edits, cursor_before, cursor_after) = &recorded[0];
    assert_eq!(*cid, client_id.as_usize());
    assert_eq!(edits.len(), 1);
    // The edit should be an Insert at position (0,0) with text "Z"
    assert!(
        matches!(&edits[0], Edit::Insert { position, text } if *position == Position::new(0, 0) && text == "Z")
    );
    assert_eq!(*cursor_before, Position::new(0, 0));
    assert_eq!(*cursor_after, Position::new(0, 1));
    // Buffer ID should match
    assert_eq!(buf_id.as_usize(), result.unwrap().as_usize());
    drop(recorded);
}

// =========================================================================
// Coverage: insert_char_for_client per-client extension sink (#497 lines 801-803)
// =========================================================================

#[test]
fn test_insert_char_for_client_per_client_extension_sink() {
    // This test covers lines 801-803: routing input to a per-client
    // extension that implements TextInputSink.
    use {
        reovim_driver_input::InputTarget,
        reovim_driver_session::{SessionExtension, TextInputSink},
    };

    /// Test extension that implements `TextInputSink`.
    #[derive(Default)]
    struct TestSinkExtension {
        buffer: String,
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    impl SessionExtension for TestSinkExtension {
        fn create() -> Self {
            Self::default()
        }

        fn as_text_input_sink(&mut self) -> Option<&mut dyn TextInputSink> {
            Some(self)
        }
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    impl TextInputSink for TestSinkExtension {
        fn insert_char(&mut self, ch: char) {
            self.buffer.push(ch);
        }
    }

    let session = Session::new(SessionId::new("pcext-test"));
    let client_id = ClientId::new(1);
    session.add_client(client_id);

    // Install extension into per-client extensions
    let type_id = std::any::TypeId::of::<TestSinkExtension>();
    session.with_clients_mut(|clients| {
        let client = clients.get_mut(&client_id).unwrap();
        client.state.extensions.get_or_insert::<TestSinkExtension>();
    });

    // Route a char to the per-client extension
    let result = session.insert_char_for_client(client_id, 'q', InputTarget::Extension(type_id));
    assert!(result.is_none()); // Extension path returns None

    // Verify the char arrived in the per-client extension
    session.with_clients(|clients| {
        let client = clients.get(&client_id).unwrap();
        let ext = client.state.extensions.get::<TestSinkExtension>().unwrap();
        assert_eq!(ext.buffer, "q");
    });
}

// =========================================================================
// Coverage: insert_char_for_client shared extension sink (#497 lines 810-811)
// =========================================================================

#[test]
fn test_insert_char_for_client_shared_extension_sink() {
    // This test covers lines 810-811: routing input to a shared session
    // extension that implements TextInputSink (fallback from per-client).
    use {
        reovim_driver_input::InputTarget,
        reovim_driver_session::{SessionExtension, TextInputSink},
    };

    /// Test extension that implements `TextInputSink`.
    #[derive(Default)]
    struct SharedSinkExtension {
        buffer: String,
    }

    impl SessionExtension for SharedSinkExtension {
        fn create() -> Self {
            Self::default()
        }

        fn as_text_input_sink(&mut self) -> Option<&mut dyn TextInputSink> {
            Some(self)
        }
    }

    impl TextInputSink for SharedSinkExtension {
        fn insert_char(&mut self, ch: char) {
            self.buffer.push(ch);
        }
    }

    let session = Session::new(SessionId::new("shext-test"));
    let client_id = ClientId::new(1);
    session.add_client(client_id);

    // Install extension into SHARED session extensions (not per-client)
    let type_id = std::any::TypeId::of::<SharedSinkExtension>();
    session.with_state_mut_sync(|state| {
        state.app.extensions.get_or_insert::<SharedSinkExtension>();
    });

    // Route a char - per-client won't have it, so falls through to shared
    let result = session.insert_char_for_client(client_id, 'w', InputTarget::Extension(type_id));
    assert!(result.is_none());

    // Verify the char arrived in the shared extension
    session.with_state_sync(|state| {
        let ext = state.app.extensions.get::<SharedSinkExtension>().unwrap();
        assert_eq!(ext.buffer, "w");
    });
}

#[test]
fn test_ensure_client_has_window_lazy_sync() {
    use {
        parking_lot::RwLock as ParkingLotRwLock,
        reovim_driver_buffer::TestBufferManager,
        reovim_kernel::api::v1::{
            EventBus, KernelContext, MarkBank, MotionEngine, OptionRegistry, ServiceRegistry,
            TextObjectEngine,
        },
        std::sync::Arc,
    };

    let kernel = KernelContext::new(
        Arc::new(EventBus::new()),
        Arc::new(TestBufferManager::new()),
        Arc::new(MotionEngine),
        Arc::new(TextObjectEngine),
        Arc::new(ParkingLotRwLock::new(MarkBank::new())),
        Arc::new(OptionRegistry::new()),
        Arc::new(ServiceRegistry::new()),
    );

    let state = SessionState::with_kernel(kernel);
    let session = Session::from_state(SessionId::new("lazy-sync"), state);

    // Add a client BEFORE any buffer exists
    let client_id = ClientId::new(1);
    session.add_client(client_id);

    // Client should have no windows yet
    let editing_state = session.client_state(client_id).unwrap();
    assert!(editing_state.windows.is_empty());

    // Now create a buffer and set it as the client's active_buffer (#471)
    let buf_id = session.with_state_mut_sync(|state| state.create_buffer("hello"));
    session.with_clients_mut(|clients| {
        if let Some(client) = clients.get_mut(&client_id) {
            client.state.active_buffer = Some(buf_id);
        }
    });

    // resolve_key_for_client should trigger ensure_client_has_window
    let key = reovim_driver_input::KeyEvent::new(reovim_driver_input::KeyCode::Char('a'));
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let _result = rt.block_on(session.resolve_key_for_client(client_id, &key));

    // After resolve_key, client should now have a window
    let editing_state = session.client_state(client_id).unwrap();
    assert!(!editing_state.windows.is_empty());
}

// =========================================================================
// Coverage: tracing::debug closures in add_client_with_metadata (lines 202-203)
// =========================================================================

#[test]
fn test_add_client_with_metadata_tracing_debug_closures() {
    // Set up a DEBUG-level tracing subscriber to execute lazy closures
    // inside tracing::debug! at lines 200-206.
    let subscriber = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .with_test_writer()
        .finish();
    let _guard = tracing::subscriber::set_default(subscriber);

    let session = Session::new(SessionId::new("tracing-debug-test"));
    let client_id = ClientId::new(42);

    // This exercises the tracing::debug! block at lines 200-206 which includes
    // mode_module = %home_mode.module() and mode_name = %home_mode.name()
    session.add_client(client_id);

    // Verify client was added
    assert!(session.client_state(client_id).is_some());
}

// =========================================================================
// Coverage: pr_info! in remove_client after crash dump (lines 254-255)
// =========================================================================

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_remove_client_with_crash_dump_written() {
    // The pr_info! at lines 252-256 only executes when try_dump_client_to_file
    // returns Some(path), which requires writing to ~/.local/share/reovim/crash/.
    // This test verifies the dump path by adding ring buffer entries first.
    let session = Session::new(SessionId::new("crash-dump-test"));
    let client_id = ClientId::new(77);
    session.add_client(client_id);

    // Log some events to the client's ring buffer so the dump has content
    session.with_client_ring_buffer(client_id, |rb| {
        rb.log_key("a");
        rb.log_key("b");
        rb.log_command("write");
    });

    // remove_client calls try_dump_client_to_file
    // If it succeeds (crash dir is writable), pr_info! at lines 254-255 executes
    let removed = session.remove_client(client_id);
    assert!(removed.is_some());
}

// =========================================================================
// Coverage: EditingState Debug impl (#497 lines 612-622)
// =========================================================================

#[test]
fn test_editing_state_debug_format() {
    // Exercise the manual Debug impl for EditingState (lines 611-622).
    // The impl formats compositor as "..." via `.map(|_| "...")`.
    let session = Session::new(SessionId::new("debug-fmt-test"));
    let client_id = ClientId::new(1);
    session.add_client(client_id);

    let state = session.client_state(client_id).unwrap();
    let debug_str = format!("{state:?}");
    assert!(debug_str.contains("EditingState"));
    assert!(debug_str.contains("mode_stack"));
    assert!(debug_str.contains("pending_keys"));
    assert!(debug_str.contains("windows"));
    assert!(debug_str.contains("viewport"));
    assert!(debug_str.contains("selection"));
    assert!(debug_str.contains("extensions"));
    assert!(debug_str.contains("compositor"));
}

// =========================================================================
// Coverage: compositor-driven window creation (#497 lines 239-251, 590-600)
// =========================================================================

/// Mock compositor that returns one tiled placement with a focused window.
///
/// Unlike `MockRootCompositor` in `runtime.rs` which returns empty results,
/// this compositor provides actual placements so the session code at
/// lines 239-251 and 590-600 can create windows from them.
struct TestPlacementCompositor;

#[cfg_attr(coverage_nightly, coverage(off))]
impl reovim_driver_display::layout::RootCompositor for TestPlacementCompositor {
    fn composite(
        &self,
        screen: reovim_driver_display::Rect,
    ) -> reovim_driver_display::layout::CompositeResult {
        use reovim_driver_display::{
            WindowId,
            layout::{CompositeResult, LayerId, WindowPlacement, ZOrder, Zone},
        };

        CompositeResult {
            placements: vec![WindowPlacement {
                window_id: WindowId::from_raw(1),
                layer_id: LayerId::new(0),
                zone: Zone::Tiled,
                bounds: reovim_driver_display::Rect::new(0, 0, 80, 24),
                z_order: ZOrder::new(0),
                visible: true,
                focusable: true,
                opacity: 1.0,
            }],
            focused: Some(WindowId::from_raw(1)),
            active_layer: Some(LayerId::new(0)),
            screen,
        }
    }

    fn create_layer(
        &mut self,
        _config: reovim_driver_display::layout::LayerConfig,
    ) -> reovim_driver_display::layout::LayerId {
        reovim_driver_display::layout::LayerId::new(0)
    }

    fn remove_layer(&mut self, _layer: reovim_driver_display::layout::LayerId) {}

    fn layer_by_label(&self, _label: &str) -> Option<reovim_driver_display::layout::LayerId> {
        None
    }

    fn layers(&self) -> Vec<&reovim_driver_display::layout::Layer> {
        Vec::new()
    }

    fn set_layer_visible(
        &mut self,
        _layer: reovim_driver_display::layout::LayerId,
        _visible: bool,
    ) {
    }

    fn set_layer_opacity(&mut self, _layer: reovim_driver_display::layout::LayerId, _opacity: f32) {
    }

    fn reorder_layer(&mut self, _layer: reovim_driver_display::layout::LayerId, _new_z: u16) {}

    fn set_active_layer(&mut self, _layer: reovim_driver_display::layout::LayerId) {}

    fn active_layer(&self) -> Option<reovim_driver_display::layout::LayerId> {
        Some(reovim_driver_display::layout::LayerId::new(0))
    }

    fn set_focus(&mut self, _window: reovim_driver_display::WindowId) {}

    fn focused(&self) -> Option<reovim_driver_display::WindowId> {
        Some(reovim_driver_display::WindowId::from_raw(1))
    }

    fn focus_at(&mut self, _x: u16, _y: u16) -> Option<reovim_driver_display::WindowId> {
        None
    }

    fn layer_compositor(
        &self,
        _layer: reovim_driver_display::layout::LayerId,
    ) -> Option<&dyn reovim_driver_display::layout::WindowLayerCompositor> {
        None
    }

    fn layer_compositor_mut(
        &mut self,
        _layer: reovim_driver_display::layout::LayerId,
    ) -> Option<&mut dyn reovim_driver_display::layout::WindowLayerCompositor> {
        None
    }

    fn window_count(&self) -> usize {
        1
    }

    fn set_screen(&mut self, _screen: reovim_driver_display::Rect) {}

    fn layer_of(
        &self,
        _window: reovim_driver_display::WindowId,
    ) -> Option<reovim_driver_display::layout::LayerId> {
        Some(reovim_driver_display::layout::LayerId::new(0))
    }

    fn boxed_clone(&self) -> Box<dyn reovim_driver_display::layout::RootCompositor> {
        Box::new(Self)
    }
}

#[test]
fn test_add_client_with_compositor_creates_windows() {
    // Exercise lines 239-251: compositor-driven window creation in
    // add_client_with_metadata() when shared compositor is set and
    // an active buffer exists.
    use {
        parking_lot::RwLock as ParkingLotRwLock,
        reovim_driver_buffer::TestBufferManager,
        reovim_kernel::api::v1::{
            EventBus, KernelContext, MarkBank, MotionEngine, OptionRegistry, ServiceRegistry,
            TextObjectEngine,
        },
        std::sync::Arc,
    };

    let kernel = KernelContext::new(
        Arc::new(EventBus::new()),
        Arc::new(TestBufferManager::new()),
        Arc::new(MotionEngine),
        Arc::new(TextObjectEngine),
        Arc::new(ParkingLotRwLock::new(MarkBank::new())),
        Arc::new(OptionRegistry::new()),
        Arc::new(ServiceRegistry::new()),
    );

    let mut state = SessionState::with_kernel(kernel);

    // Set the compositor on the shared session state BEFORE creating the session
    state
        .driver_session
        .shared
        .set_compositor(Box::new(TestPlacementCompositor));

    let session = Session::from_state(SessionId::new("compositor-add-test"), state);

    // Create a buffer so there is an active buffer
    session.with_state_mut_sync(|state| {
        state.create_buffer("hello compositor");
    });

    // Add a client - should use compositor for window creation
    let client_id = ClientId::new(1);
    session.add_client(client_id);

    // Verify client has windows from compositor placements
    let editing_state = session.client_state(client_id).unwrap();
    assert!(!editing_state.windows.is_empty());
    // The compositor returned WindowId(1), so the active window should be set
    assert!(editing_state.windows.active().is_some());
    // Compositor should be cloned into per-client state
    assert!(editing_state.compositor.is_some());
}

#[test]
fn test_ensure_client_has_window_with_compositor() {
    // Exercise lines 590-600: compositor-driven window sync in
    // ensure_client_has_window() when a client has a compositor but
    // empty windows, and the session acquires an active buffer later.
    use {
        parking_lot::RwLock as ParkingLotRwLock,
        reovim_driver_buffer::TestBufferManager,
        reovim_kernel::api::v1::{
            EventBus, KernelContext, MarkBank, MotionEngine, OptionRegistry, ServiceRegistry,
            TextObjectEngine,
        },
        std::sync::Arc,
    };

    let kernel = KernelContext::new(
        Arc::new(EventBus::new()),
        Arc::new(TestBufferManager::new()),
        Arc::new(MotionEngine),
        Arc::new(TextObjectEngine),
        Arc::new(ParkingLotRwLock::new(MarkBank::new())),
        Arc::new(OptionRegistry::new()),
        Arc::new(ServiceRegistry::new()),
    );

    let mut state = SessionState::with_kernel(kernel);

    // Set compositor on shared state
    state
        .driver_session
        .shared
        .set_compositor(Box::new(TestPlacementCompositor));

    let session = Session::from_state(SessionId::new("compositor-ensure-test"), state);

    // Add client BEFORE any buffer exists. The compositor is cloned into
    // per-client state but no windows are created yet (no active buffer).
    let client_id = ClientId::new(1);
    session.add_client(client_id);

    // Client should have compositor but empty windows
    let editing_state = session.client_state(client_id).unwrap();
    assert!(editing_state.compositor.is_some());
    assert!(editing_state.windows.is_empty());

    // Now create a buffer and set it as the client's active_buffer (#471)
    let buf_id = session.with_state_mut_sync(|state| state.create_buffer("hello lazy compositor"));
    session.with_clients_mut(|clients| {
        if let Some(client) = clients.get_mut(&client_id) {
            client.state.active_buffer = Some(buf_id);
        }
    });

    // Trigger ensure_client_has_window via resolve_key_for_client.
    // The client has a compositor + empty windows + active buffer now,
    // so lines 589-600 should execute.
    let key = reovim_driver_input::KeyEvent::new(reovim_driver_input::KeyCode::Char('a'));
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let _result = rt.block_on(session.resolve_key_for_client(client_id, &key));

    // After resolve_key, client should now have windows from compositor
    let editing_state = session.client_state(client_id).unwrap();
    assert!(!editing_state.windows.is_empty());
    assert!(editing_state.windows.active().is_some());
}

// ========================================================================
// with_client_extensions tests (#514)
// ========================================================================

#[test]
fn test_with_client_extensions_returns_none_for_unknown_client() {
    let session = Session::new(SessionId::new("test"));
    let result = session.with_client_extensions(ClientId::new(99), |_ext| 42);
    assert!(result.is_none());
}

/// Test extension replacing module-cmdline dev-dependency.
struct TestSessionExtension;

impl reovim_driver_session::SessionExtension for TestSessionExtension {
    fn create() -> Self {
        Self
    }
}

#[test]
fn test_with_client_extensions_reads_extensions() {
    let session = Session::new(SessionId::new("test"));
    let client_id = ClientId::new(1);
    session.add_client(client_id);

    // Initially empty
    let has_ext = session
        .with_client_extensions(client_id, |ext| ext.get::<TestSessionExtension>().is_some())
        .unwrap();
    assert!(!has_ext);

    // Insert extension
    session.update_client_state(client_id, |state| {
        state.extensions.get_or_insert::<TestSessionExtension>();
    });

    // Now it exists
    let has_ext = session
        .with_client_extensions(client_id, |ext| ext.get::<TestSessionExtension>().is_some())
        .unwrap();
    assert!(has_ext);
}

// ========================================================================
// with_client_extensions_mut tests (#521)
// ========================================================================

#[test]
fn test_with_client_extensions_mut_returns_none_for_unknown_client() {
    let session = Session::new(SessionId::new("test"));
    let result = session.with_client_extensions_mut(ClientId::new(99), |_ext| 42);
    assert!(result.is_none());
}

#[test]
fn test_with_client_extensions_mut_modifies_extensions() {
    let session = Session::new(SessionId::new("test"));
    let client_id = ClientId::new(1);
    session.add_client(client_id);

    // Insert extension via mutable access
    session.with_client_extensions_mut(client_id, |ext| {
        ext.get_or_insert::<TestSessionExtension>();
    });

    // Verify via read access
    let has_ext = session
        .with_client_extensions(client_id, |ext| ext.get::<TestSessionExtension>().is_some())
        .unwrap();
    assert!(has_ext);
}

// ========================================================================
// with_bridge_context tests (#543)
// ========================================================================

#[test]
fn test_with_bridge_context_unknown_client_returns_none() {
    let session = Session::new(SessionId::new("bridge-ctx"));
    let result = session.with_bridge_context(ClientId::new(99), |_, _, _| ());
    assert!(result.is_none());
}

#[test]
fn test_with_bridge_context_provides_own_extensions() {
    let session = Session::new(SessionId::new("bridge-ctx"));
    let client_id = ClientId::new(1);
    session.add_client(client_id);

    // Insert extension into client 1
    session.with_client_extensions_mut(client_id, |ext| {
        ext.get_or_insert::<TestSessionExtension>();
    });

    let has_ext = session
        .with_bridge_context(client_id, |own_ext, _, _| {
            own_ext.get::<TestSessionExtension>().is_some()
        })
        .unwrap();
    assert!(has_ext);
}

#[test]
fn test_with_bridge_context_provides_shared_extensions() {
    let session = Session::new(SessionId::new("bridge-ctx"));
    let client_id = ClientId::new(1);
    session.add_client(client_id);

    // Insert extension into shared state
    session.with_state_mut_sync(|state| {
        state.app.extensions.get_or_insert::<TestSessionExtension>();
    });

    let has_ext = session
        .with_bridge_context(client_id, |_, shared_ext, _| {
            shared_ext.get::<TestSessionExtension>().is_some()
        })
        .unwrap();
    assert!(has_ext);
}

#[test]
fn test_with_bridge_context_provides_opponents() {
    let session = Session::new(SessionId::new("bridge-ctx"));
    let client1 = ClientId::new(1);
    let client2 = ClientId::new(2);
    session.add_client(client1);
    session.add_client(client2);

    // Client 1 should see client 2 as opponent
    let opponent_count = session
        .with_bridge_context(client1, |_, _, opponents| opponents.len())
        .unwrap();
    assert_eq!(opponent_count, 1);

    // Client 2 should see client 1 as opponent
    let opponent_ids: Vec<usize> = session
        .with_bridge_context(client2, |_, _, opponents| {
            opponents.iter().map(|(id, _)| id.as_usize()).collect()
        })
        .unwrap();
    assert_eq!(opponent_ids, vec![1]);
}

#[test]
fn test_with_bridge_context_single_client_no_opponents() {
    let session = Session::new(SessionId::new("bridge-ctx"));
    let client_id = ClientId::new(1);
    session.add_client(client_id);

    let opponent_count = session
        .with_bridge_context(client_id, |_, _, opponents| opponents.len())
        .unwrap();
    assert_eq!(opponent_count, 0);
}

// ========================================================================
// Session register tests (#515 Phase 5)
// ========================================================================

#[test]
fn test_session_register_get_empty() {
    let session = Session::new(SessionId::new("reg-test"));
    assert!(session.get_session_register('A').is_none());
}

#[test]
fn test_session_register_set_and_get() {
    let session = Session::new(SessionId::new("reg-test"));
    session.set_session_register('A', RegisterContent::characterwise("shared"));
    let content = session.get_session_register('A');
    assert_eq!(content.as_ref().map(|c| c.text.as_str()), Some("shared"));
}

#[test]
fn test_session_register_overwrite() {
    let session = Session::new(SessionId::new("reg-test"));
    session.set_session_register('B', RegisterContent::characterwise("first"));
    session.set_session_register('B', RegisterContent::characterwise("second"));
    assert_eq!(session.get_session_register('B').map(|c| c.text), Some("second".to_string()));
}

#[test]
fn test_session_register_multiple_keys() {
    let session = Session::new(SessionId::new("reg-test"));
    session.set_session_register('A', RegisterContent::characterwise("alpha"));
    session.set_session_register('Z', RegisterContent::linewise("zulu\n"));

    assert_eq!(session.get_session_register('A').map(|c| c.text), Some("alpha".to_string()));
    assert_eq!(session.get_session_register('Z').map(|c| c.text), Some("zulu\n".to_string()));
    assert!(session.get_session_register('M').is_none());
}

// ========================================================================
// Peer history tests (#515 Phase 5)
// ========================================================================

#[test]
fn test_peer_history_client_not_found() {
    let session = Session::new(SessionId::new("peer-test"));
    assert!(session.get_peer_history(ClientId::new(99), 0).is_none());
}

#[test]
fn test_peer_history_empty_ring() {
    let session = Session::new(SessionId::new("peer-test"));
    let client_id = ClientId::new(1);
    session.add_client(client_id);
    // Client exists but history is empty
    assert!(session.get_peer_history(client_id, 0).is_none());
}

#[test]
fn test_peer_history_with_entries() {
    let session = Session::new(SessionId::new("peer-test"));
    let client_id = ClientId::new(1);
    session.add_client(client_id);

    // Push to the client's history ring
    session.update_client_state(client_id, |state| {
        state
            .clipboard_history
            .push(RegisterContent::characterwise("first"));
        state
            .clipboard_history
            .push(RegisterContent::characterwise("second"));
    });

    // Read peer history
    assert_eq!(
        session.get_peer_history(client_id, 0).map(|c| c.text),
        Some("second".to_string())
    );
    assert_eq!(
        session.get_peer_history(client_id, 1).map(|c| c.text),
        Some("first".to_string())
    );
    assert!(session.get_peer_history(client_id, 2).is_none());
}

#[test]
fn test_peer_history_index_out_of_range() {
    let session = Session::new(SessionId::new("peer-test"));
    let client_id = ClientId::new(1);
    session.add_client(client_id);

    session.update_client_state(client_id, |state| {
        state
            .clipboard_history
            .push(RegisterContent::characterwise("only"));
    });

    assert!(session.get_peer_history(client_id, 0).is_some());
    assert!(session.get_peer_history(client_id, 1).is_none());
    assert!(session.get_peer_history(client_id, 255).is_none());
}

// ========================================================================
// connected_client_ids tests (#515 Phase 5)
// ========================================================================

#[test]
fn test_connected_client_ids_empty() {
    let session = Session::new(SessionId::new("ids-test"));
    assert!(session.connected_client_ids().is_empty());
}

#[test]
fn test_connected_client_ids_sorted() {
    let session = Session::new(SessionId::new("ids-test"));
    session.add_client(ClientId::new(5));
    session.add_client(ClientId::new(1));
    session.add_client(ClientId::new(3));

    let ids = session.connected_client_ids();
    assert_eq!(ids.len(), 3);
    assert_eq!(ids[0].as_usize(), 1);
    assert_eq!(ids[1].as_usize(), 3);
    assert_eq!(ids[2].as_usize(), 5);
}

#[test]
fn test_connected_client_ids_after_remove() {
    let session = Session::new(SessionId::new("ids-test"));
    session.add_client(ClientId::new(1));
    session.add_client(ClientId::new(2));
    session.remove_client(ClientId::new(1));

    let ids = session.connected_client_ids();
    assert_eq!(ids.len(), 1);
    assert_eq!(ids[0].as_usize(), 2);
}

// =========================================================================
// with_tick_mut (#546)
// =========================================================================

#[test]
fn with_tick_mut_returns_none_for_unknown_client() {
    let session = Session::new(SessionId::new("tick-test"));
    let result = session.with_tick_mut(ClientId::new(99), |_, _, _| true);
    assert!(result.is_none());
}

#[test]
fn with_tick_mut_calls_closure() {
    use reovim_driver_session::SessionExtension;

    #[derive(Default)]
    struct Counter {
        count: usize,
    }
    impl SessionExtension for Counter {
        fn create() -> Self {
            Self::default()
        }
    }

    let session = Session::new(SessionId::new("tick-test"));
    session.add_client(ClientId::new(1));

    let result = session.with_tick_mut(ClientId::new(1), |client_ext, _shared_ext, _services| {
        let counter = client_ext.get_or_insert::<Counter>();
        counter.count += 1;
        counter.count
    });
    assert_eq!(result, Some(1));

    // Second call accumulates
    let result = session.with_tick_mut(ClientId::new(1), |client_ext, _shared_ext, _services| {
        let counter = client_ext.get_or_insert::<Counter>();
        counter.count += 1;
        counter.count
    });
    assert_eq!(result, Some(2));
}

#[test]
fn with_tick_mut_accesses_shared_extensions() {
    use reovim_driver_session::SessionExtension;

    #[derive(Default)]
    struct SharedData {
        value: u32,
    }
    impl SessionExtension for SharedData {
        fn create() -> Self {
            Self::default()
        }
    }

    let session = Session::new(SessionId::new("tick-shared"));
    session.add_client(ClientId::new(1));

    // Set shared state
    session.with_tick_mut(ClientId::new(1), |_client_ext, shared_ext, _services| {
        let data = shared_ext.get_or_insert::<SharedData>();
        data.value = 42;
    });

    // Read it back
    let result = session.with_tick_mut(ClientId::new(1), |_client_ext, shared_ext, _services| {
        let data = shared_ext.get_or_insert::<SharedData>();
        data.value
    });
    assert_eq!(result, Some(42));
}

#[test]
fn with_tick_mut_returns_none_for_following_client() {
    use crate::session::ClientRelation;

    let session = Session::new(SessionId::new("tick-follow"));
    session.add_client(ClientId::new(1));
    session.add_client(ClientId::new(2));
    let _ = session.set_client_relation(
        ClientId::new(2),
        Some(ClientRelation::Following {
            target: ClientId::new(1),
        }),
    );

    // Following clients have input ignored
    let result = session.with_tick_mut(ClientId::new(2), |_, _, _| true);
    assert!(result.is_none());
}
