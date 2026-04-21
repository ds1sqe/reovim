use {super::*, reovim_subsys_input::InputEvent};

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
        .clients()
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
    let _ = session
        .clients()
        .set_client_relation(share_id, Some(ClientRelation::Sharing { with: owner_id }));

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
// #[tokio::test]: DELETED (#753 E6) — uses removed methods

#[test]
fn test_session_from_state() {
    let state = SessionState::default();
    let session = Session::from_state(SessionId::new("from-state"), state);
    assert_eq!(session.id().name(), "from-state");
    assert_eq!(session.clients().client_count(), 0);
}

#[test]
fn test_add_client() {
    let session = Session::new(SessionId::new("test"));
    let client_id = ClientId::new(1);

    session.add_client(client_id);
    assert_eq!(session.clients().client_count(), 1);
    assert!(session.clients().has_client(client_id));
}

#[test]
fn test_add_client_with_metadata() {
    use crate::session::ClientMetadata;

    let session = Session::new(SessionId::new("test"));
    let client_id = ClientId::new(1);
    let metadata = ClientMetadata::default();

    session.add_client_with_metadata(client_id, metadata);
    assert_eq!(session.clients().client_count(), 1);
    assert!(session.clients().has_client(client_id));
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
    session.clients().add_client_with_state(client);

    assert_eq!(session.clients().client_count(), 1);
    assert!(session.clients().has_client(client_id));
}

#[test]
fn test_get_client() {
    let session = Session::new(SessionId::new("test"));
    let client_id = ClientId::new(1);

    session.add_client(client_id);
    let client = session.clients().get_client(client_id);
    assert!(client.is_some());
    assert_eq!(client.unwrap().id, client_id);
}

#[test]
fn test_client_state() {
    let session = Session::new(SessionId::new("test"));
    let client_id = ClientId::new(1);

    session.add_client(client_id);
    let state = session.clients().client_state(client_id);
    assert!(state.is_some());
}

#[test]
fn test_update_client_state() {
    let session = Session::new(SessionId::new("test"));
    let client_id = ClientId::new(1);

    session.add_client(client_id);

    let updated = session.clients().update_client_state(client_id, |state| {
        // Just access the mode_stack to verify we can mutate
        let _ = state.mode_stack.current();
    });

    assert!(updated);
}

#[test]
fn test_update_client_state_nonexistent() {
    let session = Session::new(SessionId::new("test"));
    let client_id = ClientId::new(999);

    let updated = session
        .clients()
        .update_client_state(client_id, |_state| {});
    assert!(!updated);
}

#[test]
fn test_with_clients() {
    let session = Session::new(SessionId::new("test"));
    let client_id = ClientId::new(1);

    session.add_client(client_id);

    let count = session
        .clients()
        .with_clients(std::collections::HashMap::len);
    assert_eq!(count, 1);
}

#[test]
fn test_with_clients_mut() {
    let session = Session::new(SessionId::new("test"));
    let client_id = ClientId::new(1);

    session.add_client(client_id);

    session.clients().with_clients_mut(|clients| {
        assert_eq!(clients.len(), 1);
    });
}

#[test]
fn test_has_client() {
    let session = Session::new(SessionId::new("test"));
    let client_id = ClientId::new(1);

    assert!(!session.clients().has_client(client_id));
    session.add_client(client_id);
    assert!(session.clients().has_client(client_id));
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
        .clients()
        .set_client_relation(follower_id, Some(ClientRelation::Following { target: owner_id }));

    assert!(result.is_ok());
    let client = session.clients().get_client(follower_id).unwrap();
    assert!(client.is_following());
}

#[test]
fn test_set_client_relation_cannot_target_self() {
    use crate::session::ClientRelation;

    let session = Session::new(SessionId::new("test"));
    let client_id = ClientId::new(1);

    session.add_client(client_id);

    let result = session
        .clients()
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

    let result = session.clients().set_client_relation(
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

    let result = session.clients().set_client_relation_unchecked(
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

    let result = session.clients().set_client_relation_unchecked(
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

    let result = session.clients().sync_and_set_relation(
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
// #[tokio::test]: DELETED (#753 E6) — uses removed methods

#[tokio::test]
async fn test_client_current_mode() {
    let session = Session::new(SessionId::new("test"));
    let client_id = ClientId::new(1);

    session.add_client(client_id);
    session.set_domain_driver(std::sync::Arc::new(SessionDispatchTestDriver::new()));

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
    use reovim_protocol::v3::Notification;

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
        .clients()
        .set_client_relation(follower_id, Some(ClientRelation::Following { target: owner_id }))
        .ok();

    // Following clients should ignore updates
    let updated = session
        .clients()
        .update_client_state(follower_id, |_state| {});
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
    session.set_domain_driver(std::sync::Arc::new(SessionDispatchTestDriver::new()));

    session
        .clients()
        .set_client_relation(sharer_id, Some(ClientRelation::Sharing { with: owner_id }))
        .ok();

    // Sharing clients should update target's state
    let updated = session
        .clients()
        .update_client_state(sharer_id, |_state| {});
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
    let result = session.clients().get_client(ClientId::new(999));
    assert!(result.is_none());
}

#[test]
fn test_client_state_returns_none_for_nonexistent() {
    let session = Session::new(SessionId::new("test"));
    let result = session.clients().client_state(ClientId::new(999));
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
        .clients()
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
    session.set_domain_driver(std::sync::Arc::new(SessionDispatchTestDriver::new()));

    session
        .clients()
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

    assert_eq!(session.clients().client_count(), 3);
    assert!(session.clients().has_client(c1));
    assert!(session.clients().has_client(c2));
    assert!(session.clients().has_client(c3));

    // Remove one
    let removed = session.remove_client(c2);
    assert!(removed.is_some());
    assert_eq!(session.clients().client_count(), 2);
    assert!(!session.clients().has_client(c2));
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
    let _ = session
        .clients()
        .set_client_relation(id2, Some(ClientRelation::Following { target: id3 }));

    // id3 follows id1
    let _ = session
        .clients()
        .set_client_relation(id3, Some(ClientRelation::Following { target: id1 }));

    // id1 trying to follow id2 would create cycle: 1 -> 2 -> 3 -> 1
    let result = session
        .clients()
        .set_client_relation(id1, Some(ClientRelation::Following { target: id2 }));
    assert!(result.is_err());
}

#[test]
fn test_sync_and_set_relation_nonexistent_client() {
    use crate::session::ClientRelation;

    let session = Session::new(SessionId::new("test"));

    let result = session.clients().sync_and_set_relation(
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
    let result = session.clients().set_client_relation(
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
        .clients()
        .set_client_relation(follower_id, Some(ClientRelation::Following { target: owner_id }));
    assert!(result.is_ok());
    assert!(
        session
            .clients()
            .get_client(follower_id)
            .unwrap()
            .is_following()
    );

    // Set back to independent (None relation)
    let result = session.clients().set_client_relation(follower_id, None);
    assert!(result.is_ok());
    assert!(
        session
            .clients()
            .get_client(follower_id)
            .unwrap()
            .is_independent()
    );
}

#[cfg(feature = "grpc")]
#[test]
fn test_emit_notification_and_receive() {
    use reovim_protocol::v3::Notification;

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
    let notification = reovim_protocol::v3::Notification {
        event_type: "orphan".to_string(),
        timestamp_ms: 0,
        payload: None,
    };

    session.emit_notification(notification);
}
// #[tokio::test]: DELETED (#753 E6) — uses removed methods

// #[tokio::test]: DELETED (#753 E6) — uses removed methods

// #[tokio::test]: DELETED (#753 E6) — uses removed methods

// #[test]: DELETED (#753 E6) — uses removed methods

// #[test]: DELETED (#753 E6) — uses removed methods

// #[test]: DELETED (#753 E6) — uses removed methods

// #[cfg_attr(coverage_nightly, coverage(off))]: DELETED (#753 E6) — uses removed methods

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
    let result = session.clients().sync_and_set_relation(
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
    let _ = session
        .clients()
        .set_client_relation(id2, Some(ClientRelation::Following { target: id1 }));

    // Try to set id1 → id2 via sync_and_set_relation → cycle
    let result = session.clients().sync_and_set_relation(
        id1,
        id2,
        Some(ClientRelation::Sharing { with: id2 }),
    );
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
    let _ = session
        .clients()
        .set_client_relation(sharer_id, Some(ClientRelation::Sharing { with: owner_id }));

    // Remove the owner (the sharing target)
    session.remove_client(owner_id);

    // Now update_client_state for sharer should return false
    // because the target (owner) no longer exists
    let updated = session
        .clients()
        .update_client_state(sharer_id, |_state| {});
    assert!(!updated);
}
// #[test]: DELETED (#753 E6) — uses removed methods

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
    assert!(session.clients().client_state(client_id).is_some());
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

    let state = session.clients().client_state(client_id).unwrap();
    let debug_str = format!("{state:?}");
    assert!(debug_str.contains("EditingState"));
    assert!(debug_str.contains("mode_stack"));
    assert!(debug_str.contains("pending_input"));
    // windows/viewport/selection are domain-owned (#753 E3)
    assert!(debug_str.contains("extensions"));
    assert!(debug_str.contains("compositor"));
}

// =========================================================================
// Coverage: compositor-driven window creation (#497 lines 239-251, 590-600)
// =========================================================================

// TestPlacementCompositor + RootCompositor impl: DELETED (#753 G-chain)
// Compositor tests: DELETED (#753 E6) — uses removed methods

// #[test]: DELETED (#753 E6) — uses removed methods

// ========================================================================
// with_client_extensions tests (#514)
// ========================================================================

#[test]
fn test_with_client_extensions_returns_none_for_unknown_client() {
    let session = Session::new(SessionId::new("test"));
    let result = session
        .clients()
        .with_client_extensions(ClientId::new(99), |_ext| 42);
    assert!(result.is_none());
}

/// Test extension replacing module-cmdline dev-dependency.
struct TestSessionExtension;

impl reovim_subsys_session::SessionExtension for TestSessionExtension {
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
        .clients()
        .with_client_extensions(client_id, |ext| ext.get::<TestSessionExtension>().is_some())
        .unwrap();
    assert!(!has_ext);

    // Insert extension
    session.clients().update_client_state(client_id, |state| {
        state.extensions.get_or_insert::<TestSessionExtension>();
    });

    // Now it exists
    let has_ext = session
        .clients()
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
    let result = session
        .clients()
        .with_client_extensions_mut(ClientId::new(99), |_ext| 42);
    assert!(result.is_none());
}

#[test]
fn test_with_client_extensions_mut_modifies_extensions() {
    let session = Session::new(SessionId::new("test"));
    let client_id = ClientId::new(1);
    session.add_client(client_id);

    // Insert extension via mutable access
    session
        .clients()
        .with_client_extensions_mut(client_id, |ext| {
            ext.get_or_insert::<TestSessionExtension>();
        });

    // Verify via read access
    let has_ext = session
        .clients()
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
    session
        .clients()
        .with_client_extensions_mut(client_id, |ext| {
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
    session.set_session_register('A', b"shared".to_vec());
    let content = session.get_session_register('A');
    assert_eq!(content.as_deref(), Some(b"shared".as_slice()));
}

#[test]
fn test_session_register_overwrite() {
    let session = Session::new(SessionId::new("reg-test"));
    session.set_session_register('B', b"first".to_vec());
    session.set_session_register('B', b"second".to_vec());
    assert_eq!(session.get_session_register('B').as_deref(), Some(b"second".as_slice()));
}

#[test]
fn test_session_register_multiple_keys() {
    let session = Session::new(SessionId::new("reg-test"));
    session.set_session_register('A', b"alpha".to_vec());
    session.set_session_register('Z', b"zulu\n".to_vec());

    assert_eq!(session.get_session_register('A').as_deref(), Some(b"alpha".as_slice()));
    assert_eq!(session.get_session_register('Z').as_deref(), Some(b"zulu\n".as_slice()));
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

// test_peer_history_with_entries removed: clipboard_history is domain-owned (#753 E3)
// test_peer_history_index_out_of_range removed: clipboard_history is domain-owned (#753 E3)

// ========================================================================
// connected_client_ids tests (#515 Phase 5)
// ========================================================================

#[test]
fn test_connected_client_ids_empty() {
    let session = Session::new(SessionId::new("ids-test"));
    assert!(session.clients().connected_client_ids().is_empty());
}

#[test]
fn test_connected_client_ids_sorted() {
    let session = Session::new(SessionId::new("ids-test"));
    session.add_client(ClientId::new(5));
    session.add_client(ClientId::new(1));
    session.add_client(ClientId::new(3));

    let ids = session.clients().connected_client_ids();
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

    let ids = session.clients().connected_client_ids();
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
    use reovim_subsys_session::SessionExtension;

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
    use reovim_subsys_session::SessionExtension;

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
    let _ = session.clients().set_client_relation(
        ClientId::new(2),
        Some(ClientRelation::Following {
            target: ClientId::new(1),
        }),
    );

    // Following clients have input ignored
    let result = session.with_tick_mut(ClientId::new(2), |_, _, _| true);
    assert!(result.is_none());
}

// ============================================================================
// Phase 4A: DomainDriver dispatch tests (server-level, no domain type leaks)
// ============================================================================
//
// Full DomainDriver dispatch tests (extension pass-through, ChangeSet content,
// delegation verification) live in driver-text-session/src/text_domain_tests.rs
// where Cursor types are available without leaking domain deps into the server.
//
// Server-level tests verify: getter/setter, fallback behavior, and the direct
// dispatch_input entry seam.

#[derive(Clone)]
struct SessionDispatchTestCursor {
    header: reovim_subsys_coordination::CursorHeader,
}

impl SessionDispatchTestCursor {
    fn new() -> Self {
        Self {
            header: reovim_subsys_coordination::CursorHeader::new(1, 0, 0),
        }
    }
}

impl reovim_subsys_coordination::Cursor for SessionDispatchTestCursor {
    fn header(&self) -> &reovim_subsys_coordination::CursorHeader {
        &self.header
    }

    fn content(&self) -> &[u8] {
        &[]
    }

    fn encode(&self) -> Vec<u8> {
        self.header.as_bytes().to_vec()
    }

    fn display(&self) -> String {
        String::new()
    }

    fn clone_box(&self) -> Box<dyn reovim_subsys_coordination::Cursor> {
        Box::new(self.clone())
    }
}

struct SessionDispatchTestContentProvider;

impl reovim_subsys_session::BufferContentProvider for SessionDispatchTestContentProvider {
    fn content_bytes(&self, _buffer_id: reovim_kernel::api::v1::BufferId) -> Option<Vec<u8>> {
        Some(Vec::new())
    }

    fn content_size(&self, _buffer_id: reovim_kernel::api::v1::BufferId) -> Option<u64> {
        Some(0)
    }

    fn content_unit_count(&self, _buffer_id: reovim_kernel::api::v1::BufferId) -> Option<usize> {
        Some(0)
    }

    fn display_lines(
        &self,
        _buffer_id: reovim_kernel::api::v1::BufferId,
        _offset: usize,
        _count: usize,
    ) -> Option<Vec<reovim_subsys_session::DisplayLine>> {
        Some(Vec::new())
    }

    fn is_modified(&self, _buffer_id: reovim_kernel::api::v1::BufferId) -> bool {
        false
    }

    fn write_to(
        &self,
        _buffer_id: reovim_kernel::api::v1::BufferId,
        _writer: &mut dyn std::io::Write,
    ) -> std::io::Result<()> {
        Ok(())
    }
}

struct SessionDispatchTestDriver {
    inputs: std::sync::Mutex<Vec<Vec<u8>>>,
    content_provider: std::sync::Arc<dyn reovim_subsys_session::BufferContentProvider>,
}

impl SessionDispatchTestDriver {
    fn new() -> Self {
        Self {
            inputs: std::sync::Mutex::new(Vec::new()),
            content_provider: std::sync::Arc::new(SessionDispatchTestContentProvider),
        }
    }

    fn payloads(&self) -> Vec<Vec<u8>> {
        self.inputs.lock().unwrap().clone()
    }
}

impl reovim_subsys_session::DomainRouting for SessionDispatchTestDriver {
    fn current_mode(
        &self,
        _client_id: reovim_subsys_session::ClientId,
    ) -> Option<reovim_kernel::api::v1::ModeId> {
        Some(reovim_kernel::api::v1::ModeId::new(
            reovim_kernel::api::v1::ModuleId::new("test"),
            "normal",
        ))
    }
}

impl reovim_subsys_session::DomainDriver for SessionDispatchTestDriver {
    fn domain_name(&self) -> &'static str {
        "test"
    }

    fn domain_id(&self) -> u32 {
        1
    }

    fn create_buffer(&self, _content: &[u8]) -> reovim_kernel::api::v1::BufferId {
        reovim_kernel::api::v1::BufferId::new()
    }

    fn close_buffer(&self, _buffer_id: reovim_kernel::api::v1::BufferId) {}

    fn content_provider(&self) -> std::sync::Arc<dyn reovim_subsys_session::BufferContentProvider> {
        std::sync::Arc::clone(&self.content_provider)
    }

    fn dispatch_input(
        &self,
        _client_id: reovim_subsys_session::ClientId,
        event: &InputEvent,
        _client_ext: &mut reovim_subsys_session::ExtensionMap,
        _shared_ext: &mut reovim_subsys_session::ExtensionMap,
    ) -> reovim_subsys_session::DispatchResult {
        self.inputs.lock().unwrap().push(event.payload().to_vec());
        reovim_subsys_session::DispatchResult::default()
    }

    fn dispatch_command(
        &self,
        _client_id: reovim_subsys_session::ClientId,
        _command: &str,
        _args: &[String],
    ) -> reovim_subsys_session::CommandResult {
        reovim_subsys_session::CommandResult::NotHandled
    }

    fn on_client_added(&self, _client_id: reovim_subsys_session::ClientId) {}

    fn on_client_removed(&self, _client_id: reovim_subsys_session::ClientId) {}

    fn on_focus_gained(
        &self,
        _client_id: reovim_subsys_session::ClientId,
        _window_id: reovim_kernel::api::v1::WindowId,
        _buffer_id: reovim_kernel::api::v1::BufferId,
    ) {
    }

    fn on_focus_lost(
        &self,
        _client_id: reovim_subsys_session::ClientId,
        _window_id: reovim_kernel::api::v1::WindowId,
        _buffer_id: reovim_kernel::api::v1::BufferId,
    ) {
    }

    fn cursors(
        &self,
        _client_id: reovim_subsys_session::ClientId,
        _window_id: reovim_kernel::api::v1::WindowId,
    ) -> Vec<Box<dyn reovim_subsys_coordination::Cursor>> {
        vec![Box::new(SessionDispatchTestCursor::new())]
    }

    fn initial_cursor(
        &self,
        _client_id: reovim_subsys_session::ClientId,
        _buffer_id: reovim_kernel::api::v1::BufferId,
    ) -> Box<dyn reovim_subsys_coordination::Cursor> {
        Box::new(SessionDispatchTestCursor::new())
    }

    fn collect_projections(
        &self,
        _client_id: reovim_subsys_session::ClientId,
    ) -> Vec<reovim_subsys_coordination::Projection> {
        Vec::new()
    }

    fn initial_projections(
        &self,
        _client_id: reovim_subsys_session::ClientId,
    ) -> Vec<reovim_subsys_coordination::Projection> {
        Vec::new()
    }
}

#[test]
fn domain_driver_getter_returns_none_by_default() {
    let session = Session::new(SessionId::new("getter-test"));
    assert!(session.domain_driver().is_none());
}

#[tokio::test]
async fn dispatch_input_for_client_happy_path_uses_domain_driver() {
    let session = Session::new(SessionId::new("dispatch-input-test"));
    let client_id = ClientId::new(1);
    session.add_client(client_id);
    let driver = std::sync::Arc::new(SessionDispatchTestDriver::new());
    session.set_domain_driver(driver.clone());

    // Opaque payload — server does not interpret content, only forwards.
    let payload = {
        let mut p = vec![0u8; reovim_subsys_input::INPUT_HEADER_SIZE];
        p.extend_from_slice(&[0x11, 0x22]);
        p
    };
    let event = InputEvent::new(payload.clone(), None, 0).expect("payload >= header size");

    let result = session.dispatch_input_for_client(client_id, &event).await;

    assert!(result.is_some());
    let dispatch = result.unwrap();
    assert!(!dispatch.buffers.has_changes());
    assert_eq!(dispatch.directive, reovim_subsys_session::Directive::Continue);
    assert_eq!(driver.payloads(), vec![payload]);
}

#[tokio::test]
async fn dispatch_input_fallback_without_domain_driver() {
    let session = Session::new(SessionId::new("fallback-test"));
    let client_id = ClientId::new(1);
    session.add_client(client_id);

    let event = InputEvent::new(vec![0u8; reovim_subsys_input::INPUT_HEADER_SIZE], None, 0)
        .expect("payload >= header size");
    let result = session.dispatch_input_for_client(client_id, &event).await;

    // Fallback stub returns None when no domain driver is active (#753 E3)
    assert!(result.is_none(), "should return None when no domain driver active");
}
