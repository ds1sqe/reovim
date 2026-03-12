use std::collections::HashMap;

use {
    reovim_driver_session::{CursorPosition, SelectionMode},
    reovim_kernel::api::v1::{ModeStack, ModuleId},
};

use super::{
    Client, ClientId, ClientMetadata, ClientRelation, ClientSelection, EditingState,
    TransitionResult,
};

fn test_mode_stack() -> ModeStack {
    let mode = reovim_kernel::api::v1::ModeId::new(ModuleId::new("test"), "normal");
    ModeStack::new(mode)
}

fn test_metadata() -> ClientMetadata {
    ClientMetadata::new("tui", "test-laptop")
}

#[test]
fn test_client_new_independent() {
    let client = Client::new(ClientId::new(1), test_metadata());
    assert!(client.is_independent());
    assert!(!client.is_following());
    assert!(!client.is_sharing());
    assert!(client.target_id().is_none());
    assert_eq!(client.id, ClientId::new(1));
}

#[test]
fn test_client_with_mode_stack() {
    let mode_stack = test_mode_stack();
    let client = Client::with_mode_stack(ClientId::new(1), test_metadata(), mode_stack.clone());

    assert!(client.is_independent());
    assert_eq!(client.state.mode_stack.current(), mode_stack.current());
}

#[test]
fn test_client_is_following() {
    let mut client = Client::new(ClientId::new(1), test_metadata());
    client.relation = Some(ClientRelation::Following {
        target: ClientId::new(42),
    });

    assert!(client.is_following());
    assert!(!client.is_independent());
    assert!(!client.is_sharing());
    assert_eq!(client.target_id(), Some(ClientId::new(42)));
}

#[test]
fn test_client_is_sharing() {
    let mut client = Client::new(ClientId::new(1), test_metadata());
    client.relation = Some(ClientRelation::Sharing {
        with: ClientId::new(42),
    });

    assert!(client.is_sharing());
    assert!(!client.is_independent());
    assert!(!client.is_following());
    assert_eq!(client.target_id(), Some(ClientId::new(42)));
}

#[test]
fn test_effective_state_independent() {
    let client = Client::new(ClientId::new(1), test_metadata());
    let clients = HashMap::new();

    let state = client.effective_state(&clients);
    assert!(state.is_some());
}

#[test]
fn test_effective_state_following() {
    let owner_id = ClientId::new(1);
    let follower_id = ClientId::new(2);

    let mut clients = HashMap::new();
    clients.insert(owner_id, Client::new(owner_id, test_metadata()));

    let mut follower = Client::new(follower_id, test_metadata());
    follower.relation = Some(ClientRelation::Following { target: owner_id });
    clients.insert(follower_id, follower);

    let follower = clients.get(&follower_id).unwrap();
    let state = follower.effective_state(&clients);

    // Should return the owner's state
    assert!(state.is_some());
}

#[test]
fn test_effective_state_sharing() {
    let owner_id = ClientId::new(1);
    let sharer_id = ClientId::new(2);

    let mut clients = HashMap::new();
    clients.insert(owner_id, Client::new(owner_id, test_metadata()));

    let mut sharer = Client::new(sharer_id, test_metadata());
    sharer.relation = Some(ClientRelation::Sharing { with: owner_id });
    clients.insert(sharer_id, sharer);

    let sharer = clients.get(&sharer_id).unwrap();
    let state = sharer.effective_state(&clients);

    // Should return the owner's state
    assert!(state.is_some());
}

#[test]
fn test_effective_state_missing_target() {
    let mut client = Client::new(ClientId::new(1), test_metadata());
    client.relation = Some(ClientRelation::Following {
        target: ClientId::new(999),
    }); // Non-existent
    let clients = HashMap::new();

    let state = client.effective_state(&clients);
    assert!(state.is_none());
}

#[test]
fn test_effective_state_chain() {
    // Test: Client 3 follows Client 2 who follows Client 1 (independent)
    let id1 = ClientId::new(1);
    let id2 = ClientId::new(2);
    let id3 = ClientId::new(3);

    let mut clients = HashMap::new();
    clients.insert(id1, Client::new(id1, test_metadata()));

    let mut mid = Client::new(id2, test_metadata());
    mid.relation = Some(ClientRelation::Following { target: id1 });
    clients.insert(id2, mid);

    let mut end = Client::new(id3, test_metadata());
    end.relation = Some(ClientRelation::Following { target: id2 });
    clients.insert(id3, end);

    let end_client = clients.get(&id3).unwrap();
    let state = end_client.effective_state(&clients);

    // Should resolve through the chain to owner's state
    assert!(state.is_some());
}

#[test]
fn test_effective_state_cycle_protection() {
    // Create a cycle: 1 → 2 → 3 → 1
    let id1 = ClientId::new(1);
    let id2 = ClientId::new(2);
    let id3 = ClientId::new(3);

    let mut clients = HashMap::new();

    let mut c1 = Client::new(id1, test_metadata());
    c1.relation = Some(ClientRelation::Following { target: id3 });
    clients.insert(id1, c1);

    let mut c2 = Client::new(id2, test_metadata());
    c2.relation = Some(ClientRelation::Following { target: id1 });
    clients.insert(id2, c2);

    let mut c3 = Client::new(id3, test_metadata());
    c3.relation = Some(ClientRelation::Following { target: id2 });
    clients.insert(id3, c3);

    let client = clients.get(&id1).unwrap();
    let state = client.effective_state(&clients);

    // Should return None due to cycle (depth limit exceeded)
    assert!(state.is_none());
}

#[test]
fn test_effective_state_mut_following_ignored() {
    let owner_id = ClientId::new(1);
    let follower_id = ClientId::new(2);

    let mut clients = HashMap::new();
    clients.insert(owner_id, Client::new(owner_id, test_metadata()));

    let mut follower = Client::new(follower_id, test_metadata());
    follower.relation = Some(ClientRelation::Following { target: owner_id });

    // Following clients should get None (input ignored)
    let state = follower.effective_state_mut(&mut clients);
    assert!(state.is_none());
}

#[test]
fn test_editing_state_default() {
    let state = EditingState::default();

    assert!(state.pending_keys.is_empty());
    assert!(state.windows.is_empty()); // Per-client windows (#471)
    assert!(state.selection.is_none());
}

#[test]
fn test_editing_state_with_mode_stack() {
    let mode_stack = test_mode_stack();
    let state = EditingState::with_mode_stack(mode_stack.clone());

    assert_eq!(state.mode_stack.current(), mode_stack.current());
}

#[test]
fn test_editing_state_clear_pending_keys() {
    let mut state = EditingState::default();
    state.pending_keys.push("d".to_string());
    state.pending_keys.push("w".to_string());

    assert!(!state.pending_keys.is_empty());
    state.clear_pending_keys();
    assert!(state.pending_keys.is_empty());
}

#[test]
fn test_client_selection() {
    let anchor = CursorPosition::new(0, 0);
    let cursor = CursorPosition::new(5, 10);
    let selection = ClientSelection::new(anchor, cursor, SelectionMode::Character);

    assert_eq!(selection.anchor, anchor);
    assert_eq!(selection.cursor, cursor);
    assert_eq!(selection.mode, SelectionMode::Character);

    let driver_sel = selection.to_driver_selection();
    assert_eq!(driver_sel.mode, SelectionMode::Character);
}

#[test]
fn test_client_metadata_new() {
    let metadata = ClientMetadata::new("tui", "my-laptop");
    assert_eq!(metadata.client_type, "tui");
    assert_eq!(metadata.display_name, "my-laptop");
    assert!(metadata.joined_at_ms > 0);
}

#[test]
fn test_client_relation_following() {
    let relation = ClientRelation::Following {
        target: ClientId::new(42),
    };
    assert!(relation.is_following());
    assert!(!relation.is_sharing());
    assert_eq!(relation.target_id(), ClientId::new(42));
}

#[test]
fn test_client_relation_sharing() {
    let relation = ClientRelation::Sharing {
        with: ClientId::new(42),
    };
    assert!(relation.is_sharing());
    assert!(!relation.is_following());
    assert_eq!(relation.target_id(), ClientId::new(42));
}

// =========================================================================
// Phase 2: State Machine Tests
// =========================================================================

#[test]
fn test_transition_independent_to_following() {
    let id1 = ClientId::new(1);
    let id2 = ClientId::new(2);

    let mut clients = HashMap::new();
    clients.insert(id1, Client::new(id1, test_metadata()));
    clients.insert(id2, Client::new(id2, test_metadata()));

    let mut client1 = clients.remove(&id1).unwrap();
    assert!(client1.is_independent());

    let result =
        client1.try_set_relation(Some(ClientRelation::Following { target: id2 }), &clients);
    assert!(result.is_ok());
    assert!(client1.is_following());
    assert_eq!(client1.target_id(), Some(id2));
}

#[test]
fn test_transition_independent_to_sharing() {
    let id1 = ClientId::new(1);
    let id2 = ClientId::new(2);

    let mut clients = HashMap::new();
    clients.insert(id1, Client::new(id1, test_metadata()));
    clients.insert(id2, Client::new(id2, test_metadata()));

    let mut client1 = clients.remove(&id1).unwrap();
    assert!(client1.is_independent());

    let result =
        client1.try_set_relation(Some(ClientRelation::Sharing { with: id2 }), &clients);
    assert!(result.is_ok());
    assert!(client1.is_sharing());
    assert_eq!(client1.target_id(), Some(id2));
}

#[test]
fn test_transition_following_to_independent() {
    let id1 = ClientId::new(1);
    let id2 = ClientId::new(2);

    let mut clients = HashMap::new();
    clients.insert(id2, Client::new(id2, test_metadata()));

    let mut client1 = Client::new(id1, test_metadata());
    client1.relation = Some(ClientRelation::Following { target: id2 });
    assert!(client1.is_following());

    let result = client1.try_set_relation(None, &clients);
    assert!(result.is_ok());
    assert!(client1.is_independent());
}

#[test]
fn test_transition_sharing_to_independent() {
    let id1 = ClientId::new(1);
    let id2 = ClientId::new(2);

    let mut clients = HashMap::new();
    clients.insert(id2, Client::new(id2, test_metadata()));

    let mut client1 = Client::new(id1, test_metadata());
    client1.relation = Some(ClientRelation::Sharing { with: id2 });
    assert!(client1.is_sharing());

    let result = client1.try_set_relation(None, &clients);
    assert!(result.is_ok());
    assert!(client1.is_independent());
}

#[test]
fn test_transition_sharing_to_following() {
    let id1 = ClientId::new(1);
    let id2 = ClientId::new(2);

    let mut clients = HashMap::new();
    clients.insert(id2, Client::new(id2, test_metadata()));

    let mut client1 = Client::new(id1, test_metadata());
    client1.relation = Some(ClientRelation::Sharing { with: id2 });

    let result =
        client1.try_set_relation(Some(ClientRelation::Following { target: id2 }), &clients);
    assert!(result.is_ok());
    assert!(client1.is_following());
}

#[test]
fn test_transition_cannot_follow_self() {
    let id1 = ClientId::new(1);
    let clients = HashMap::new();

    let mut client1 = Client::new(id1, test_metadata());
    let result =
        client1.try_set_relation(Some(ClientRelation::Following { target: id1 }), &clients);
    assert_eq!(result, TransitionResult::CannotTargetSelf);
    assert!(client1.is_independent()); // Relation unchanged
}

#[test]
fn test_transition_cannot_share_with_self() {
    let id1 = ClientId::new(1);
    let clients = HashMap::new();

    let mut client1 = Client::new(id1, test_metadata());
    let result =
        client1.try_set_relation(Some(ClientRelation::Sharing { with: id1 }), &clients);
    assert_eq!(result, TransitionResult::CannotTargetSelf);
    assert!(client1.is_independent()); // Relation unchanged
}

#[test]
fn test_transition_target_not_found() {
    let id1 = ClientId::new(1);
    let id999 = ClientId::new(999);
    let clients = HashMap::new();

    let mut client1 = Client::new(id1, test_metadata());
    let result =
        client1.try_set_relation(Some(ClientRelation::Following { target: id999 }), &clients);
    assert_eq!(result, TransitionResult::TargetNotFound(id999));
    assert!(client1.is_independent()); // Relation unchanged
}

#[test]
fn test_transition_prevents_cycle() {
    // Setup: id2 follows id1 (independent)
    // Attempt: id1 follows id2 → should fail (would create cycle)
    let id1 = ClientId::new(1);
    let id2 = ClientId::new(2);

    let mut clients = HashMap::new();

    let mut client2 = Client::new(id2, test_metadata());
    client2.relation = Some(ClientRelation::Following { target: id1 });
    clients.insert(id2, client2);

    let mut client1 = Client::new(id1, test_metadata());
    let result =
        client1.try_set_relation(Some(ClientRelation::Following { target: id2 }), &clients);
    assert_eq!(result, TransitionResult::WouldCreateCycle);
    assert!(client1.is_independent()); // Relation unchanged
}

#[test]
fn test_transition_prevents_longer_cycle() {
    // Setup: id2 → id3 → id1 (chain)
    // Attempt: id1 → id2 → should fail (would create cycle)
    let id1 = ClientId::new(1);
    let id2 = ClientId::new(2);
    let id3 = ClientId::new(3);

    let mut clients = HashMap::new();

    let mut client3 = Client::new(id3, test_metadata());
    client3.relation = Some(ClientRelation::Following { target: id1 });
    clients.insert(id3, client3);

    let mut client2 = Client::new(id2, test_metadata());
    client2.relation = Some(ClientRelation::Following { target: id3 });
    clients.insert(id2, client2);

    let mut client1 = Client::new(id1, test_metadata());
    let result =
        client1.try_set_relation(Some(ClientRelation::Following { target: id2 }), &clients);
    assert_eq!(result, TransitionResult::WouldCreateCycle);
}

#[test]
fn test_transition_result_is_ok() {
    assert!(TransitionResult::Ok.is_ok());
    assert!(!TransitionResult::CannotTargetSelf.is_ok());
    assert!(!TransitionResult::WouldCreateCycle.is_ok());
}

#[test]
fn test_transition_result_requires_cursor_sync() {
    let result = TransitionResult::RequiresCursorSync {
        current: CursorPosition::default(),
        target: CursorPosition::new(5, 10),
    };
    assert!(result.requires_cursor_sync());
    assert!(!TransitionResult::Ok.requires_cursor_sync());
}

#[test]
fn test_sync_cursor_to() {
    let id1 = ClientId::new(1);
    let id2 = ClientId::new(2);

    // Create target client with a window and cursor
    let mut target = Client::with_mode_stack_and_window(
        id2,
        test_metadata(),
        test_mode_stack(),
        reovim_driver_session::Window::new(),
    );
    // Set target's cursor position
    if let Some(w) = target.state.windows.active_mut() {
        w.cursor = CursorPosition::new(10, 20);
    }

    // Create source client with a window
    let mut source = Client::with_mode_stack_and_window(
        id1,
        test_metadata(),
        test_mode_stack(),
        reovim_driver_session::Window::new(),
    );
    // Verify initial cursor is at default
    assert_eq!(source.state.windows.active().unwrap().cursor, CursorPosition::default());

    // Sync cursor
    source.sync_cursor_to(&target);

    // Verify cursor was synced
    assert_eq!(source.state.windows.active().unwrap().cursor, CursorPosition::new(10, 20));
}

#[test]
fn test_set_relation_unchecked() {
    let id1 = ClientId::new(1);
    let id2 = ClientId::new(2);

    let mut client = Client::new(id1, test_metadata());
    assert!(client.is_independent());

    // Set relation unchecked (no validation)
    client.set_relation_unchecked(Some(ClientRelation::Following { target: id2 }));
    assert!(client.is_following());
    assert_eq!(client.target_id(), Some(id2));

    // Back to independent
    client.set_relation_unchecked(None);
    assert!(client.is_independent());
}

// =========================================================================
// Coverage: RequiresCursorSync path
// =========================================================================

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_validate_following_to_sharing_requires_cursor_sync() {
    // Setup: client1 follows client2, then tries to upgrade to Sharing
    // with mismatched cursors -> RequiresCursorSync
    let id1 = ClientId::new(1);
    let id2 = ClientId::new(2);

    let mut clients = HashMap::new();

    // Client 2 (target) with a window and cursor at (5, 10)
    let mut target = Client::with_mode_stack_and_window(
        id2,
        test_metadata(),
        test_mode_stack(),
        reovim_driver_session::Window::new(),
    );
    if let Some(w) = target.state.windows.active_mut() {
        w.cursor = CursorPosition::new(5, 10);
    }
    clients.insert(id2, target);

    // Client 1 (follower) with a window and cursor at default (0, 0)
    let mut follower = Client::with_mode_stack_and_window(
        id1,
        test_metadata(),
        test_mode_stack(),
        reovim_driver_session::Window::new(),
    );
    follower.relation = Some(ClientRelation::Following { target: id2 });

    // Attempt upgrade from Following(id2) to Sharing(id2) with cursor mismatch
    let result = Client::validate_relation_change(
        &follower,
        Some(ClientRelation::Sharing { with: id2 }),
        &clients,
    );

    assert!(result.requires_cursor_sync());
    match result {
        TransitionResult::RequiresCursorSync { current, target } => {
            assert_eq!(current, CursorPosition::default());
            assert_eq!(target, CursorPosition::new(5, 10));
        }
        other => panic!("Expected RequiresCursorSync, got {other:?}"),
    }
}

#[test]
fn test_validate_following_to_sharing_same_cursor_ok() {
    // Same target, cursors already match -> Ok
    let id1 = ClientId::new(1);
    let id2 = ClientId::new(2);

    let mut clients = HashMap::new();

    let target = Client::with_mode_stack_and_window(
        id2,
        test_metadata(),
        test_mode_stack(),
        reovim_driver_session::Window::new(),
    );
    clients.insert(id2, target);

    let mut follower = Client::with_mode_stack_and_window(
        id1,
        test_metadata(),
        test_mode_stack(),
        reovim_driver_session::Window::new(),
    );
    follower.relation = Some(ClientRelation::Following { target: id2 });

    // Cursors both at default (0,0) -> should succeed
    let result = Client::validate_relation_change(
        &follower,
        Some(ClientRelation::Sharing { with: id2 }),
        &clients,
    );
    assert!(result.is_ok());
}

#[test]
fn test_validate_following_to_sharing_different_target_no_sync() {
    // Following target A, trying to Share with target B -> no cursor sync check
    let id1 = ClientId::new(1);
    let id2 = ClientId::new(2);
    let id3 = ClientId::new(3);

    let mut clients = HashMap::new();
    clients.insert(id2, Client::new(id2, test_metadata()));
    clients.insert(id3, Client::new(id3, test_metadata()));

    let mut follower = Client::new(id1, test_metadata());
    follower.relation = Some(ClientRelation::Following { target: id2 });

    // Following id2, but sharing with id3 -> different targets, no sync check
    let result = Client::validate_relation_change(
        &follower,
        Some(ClientRelation::Sharing { with: id3 }),
        &clients,
    );
    assert!(result.is_ok());
}

// =========================================================================
// Coverage: effective_state_mut Sharing case
// =========================================================================

#[test]
fn test_effective_state_mut_sharing_returns_target_state() {
    let owner_id = ClientId::new(1);
    let sharer_id = ClientId::new(2);

    let mut clients = HashMap::new();
    clients.insert(owner_id, Client::new(owner_id, test_metadata()));

    let mut sharer = Client::new(sharer_id, test_metadata());
    sharer.relation = Some(ClientRelation::Sharing { with: owner_id });

    // Sharing clients should get mutable access to owner's state
    let state = sharer.effective_state_mut(&mut clients);
    assert!(state.is_some());
}

#[test]
fn test_effective_state_mut_independent_returns_none() {
    let id = ClientId::new(1);
    let client = Client::new(id, test_metadata());
    let mut clients = HashMap::new();

    // Independent returns None (caller should use client.state directly)
    let state = client.effective_state_mut(&mut clients);
    assert!(state.is_none());
}

#[test]
fn test_effective_state_mut_sharing_missing_target() {
    let sharer_id = ClientId::new(1);
    let missing_id = ClientId::new(999);

    let mut sharer = Client::new(sharer_id, test_metadata());
    sharer.relation = Some(ClientRelation::Sharing { with: missing_id });

    let mut clients = HashMap::new();
    let state = sharer.effective_state_mut(&mut clients);
    assert!(state.is_none());
}

// =========================================================================
// Coverage: EditingState::clone
// =========================================================================

#[test]
fn test_editing_state_clone() {
    let mut state = EditingState::default();
    state.pending_keys.push("d".to_string());
    state.selection = Some(ClientSelection::new(
        CursorPosition::new(0, 0),
        CursorPosition::new(1, 5),
        SelectionMode::Character,
    ));

    let cloned = state.clone();

    // Cloned should have same mode stack
    assert_eq!(cloned.mode_stack.current().name(), state.mode_stack.current().name());
    // Cloned should have same pending keys
    assert!(!cloned.pending_keys.is_empty());
    // Cloned should have same selection
    assert!(cloned.selection.is_some());
    // Cloned should have FRESH extensions (not copied)
    // Extensions are intentionally not cloned for isolation
    let _ = cloned.extensions;
}

// =========================================================================
// Coverage: ClientMetadata::with_timestamp
// =========================================================================

#[test]
fn test_client_metadata_with_timestamp() {
    let metadata = super::ClientMetadata::with_timestamp("android", "phone", 1_700_000_000_000);
    assert_eq!(metadata.client_type, "android");
    assert_eq!(metadata.display_name, "phone");
    assert_eq!(metadata.joined_at_ms, 1_700_000_000_000);
}

// =========================================================================
// Coverage: ClientRelation target_id for both variants
// =========================================================================

#[test]
fn test_client_ring_buffer_accessor() {
    let client = Client::new(ClientId::new(1), test_metadata());
    let rb = client.ring_buffer();
    // Just verify accessor works
    let _ = rb;
}

// =========================================================================
// Coverage: EditingState::with_mode_stack_and_window
// =========================================================================

#[test]
fn test_editing_state_with_mode_stack_and_window() {
    let mode_stack = test_mode_stack();
    let window = reovim_driver_session::Window::new();
    let state = EditingState::with_mode_stack_and_window(mode_stack.clone(), window);

    assert_eq!(state.mode_stack.current().name(), mode_stack.current().name());
    assert!(!state.windows.is_empty());
    assert!(state.selection.is_none());
    assert!(state.pending_keys.is_empty());
}

#[test]
fn test_editing_state_current_mode() {
    let state = EditingState::default();
    let mode = state.current_mode();
    assert_eq!(mode.name(), "normal");
}

// =========================================================================
// Coverage: ClientSelection::to_driver_selection all modes
// =========================================================================

#[test]
fn test_client_selection_line_mode() {
    let selection = ClientSelection::new(
        CursorPosition::new(0, 0),
        CursorPosition::new(3, 0),
        SelectionMode::Line,
    );
    let driver_sel = selection.to_driver_selection();
    assert_eq!(driver_sel.mode, SelectionMode::Line);
}

#[test]
fn test_client_selection_block_mode() {
    let selection = ClientSelection::new(
        CursorPosition::new(1, 2),
        CursorPosition::new(3, 4),
        SelectionMode::Block,
    );
    let driver_sel = selection.to_driver_selection();
    assert_eq!(driver_sel.mode, SelectionMode::Block);
}

// =========================================================================
// Coverage: cycle detection depth limit
// =========================================================================

#[test]
fn test_cycle_detection_depth_limit() {
    // Create a very long chain to test depth limit (10 hops)
    let mut clients = HashMap::new();

    // Chain: id0 → id1 → id2 → ... → id10 → id11
    for i in 0..12 {
        let id = ClientId::new(i);
        let mut client = Client::new(id, test_metadata());
        if i > 0 {
            client.relation = Some(ClientRelation::Following {
                target: ClientId::new(i + 1),
            });
        }
        clients.insert(id, client);
    }

    // Try to make id11 point to id0 (would create cycle if depth wasn't limited)
    let mut last_client = clients.remove(&ClientId::new(11)).unwrap();
    let result = last_client.try_set_relation(
        Some(ClientRelation::Following {
            target: ClientId::new(0),
        }),
        &clients,
    );

    // Should succeed because depth limit prevents cycle detection at depth 10
    assert!(result.is_ok());
}

#[test]
fn test_cycle_detection_missing_intermediate() {
    // Chain with missing intermediate: id1 → id2 (missing) → ...
    let id1 = ClientId::new(1);
    let id2 = ClientId::new(2);

    let mut clients = HashMap::new();

    let mut client1 = Client::new(id1, test_metadata());
    client1.relation = Some(ClientRelation::Following { target: id2 });
    clients.insert(id1, client1);

    // id2 doesn't exist, so cycle check should return false (no cycle)
    let mut test_client = Client::new(ClientId::new(99), test_metadata());
    let result =
        test_client.try_set_relation(Some(ClientRelation::Following { target: id1 }), &clients);

    // Should succeed - can't detect cycle through missing client
    assert!(result.is_ok());
}

#[test]
fn test_effective_state_mut_independent_borrow_semantics() {
    // Test that independent clients return None from effective_state_mut
    // (because of Rust borrow rules - can't return &mut self.state)
    let id = ClientId::new(1);
    let client = Client::new(id, test_metadata());
    let mut clients = HashMap::new();

    let state = client.effective_state_mut(&mut clients);
    assert!(state.is_none());

    // Caller should use client.state directly for independent clients
    assert!(client.state.pending_keys.is_empty());
}

#[test]
fn test_sync_cursor_to_no_windows() {
    // Test sync_cursor_to when one or both clients have no windows
    let id1 = ClientId::new(1);
    let id2 = ClientId::new(2);

    let mut source = Client::new(id1, test_metadata());
    let target = Client::new(id2, test_metadata());

    // Both have no windows - should not crash
    source.sync_cursor_to(&target);

    // Verify nothing changed (no windows to sync)
    assert!(source.state.windows.active().is_none());
}

#[test]
fn test_client_metadata_default() {
    let metadata = ClientMetadata::default();
    assert_eq!(metadata.client_type, "unknown");
    assert_eq!(metadata.display_name, "unknown");
    assert!(metadata.joined_at_ms > 0);
}

#[test]
fn test_validate_relation_change_independent_to_none() {
    // Changing from independent (None) to independent (None) should succeed
    let id = ClientId::new(1);
    let client = Client::new(id, test_metadata());
    let clients = HashMap::new();

    let result = Client::validate_relation_change(&client, None, &clients);
    assert!(result.is_ok());
}

#[test]
fn test_effective_state_sharing_chain() {
    // Test: Client 3 shares with Client 2 who shares with Client 1
    let id1 = ClientId::new(1);
    let id2 = ClientId::new(2);
    let id3 = ClientId::new(3);

    let mut clients = HashMap::new();
    clients.insert(id1, Client::new(id1, test_metadata()));

    let mut mid = Client::new(id2, test_metadata());
    mid.relation = Some(ClientRelation::Sharing { with: id1 });
    clients.insert(id2, mid);

    let mut end = Client::new(id3, test_metadata());
    end.relation = Some(ClientRelation::Sharing { with: id2 });
    clients.insert(id3, end);

    let end_client = clients.get(&id3).unwrap();
    let state = end_client.effective_state(&clients);

    // Should resolve through the chain to owner's state
    assert!(state.is_some());
}

#[test]
fn test_client_with_mode_stack_and_window() {
    let id = ClientId::new(1);
    let mode_stack = test_mode_stack();
    let window = reovim_driver_session::Window::new();

    let client =
        Client::with_mode_stack_and_window(id, test_metadata(), mode_stack.clone(), window);

    assert!(client.is_independent());
    assert_eq!(client.state.mode_stack.current().name(), mode_stack.current().name());
    assert!(!client.state.windows.is_empty());
    assert_eq!(client.id, id);
}

#[test]
fn test_would_create_cycle_impl_depth_limit() {
    use super::would_create_cycle;

    // Build a long chain: A -> B -> C -> ... that exceeds depth 10
    let mut clients = HashMap::new();
    for i in 0..12 {
        let mut client = Client::new(ClientId::new(i), test_metadata());
        if i < 11 {
            client.relation = Some(ClientRelation::Following {
                target: ClientId::new(i + 1),
            });
        }
        clients.insert(ClientId::new(i), client);
    }
    // No actual cycle, but chain length > 10.
    // would_create_cycle starts traversing from target (1) looking for start (0).
    // Chain: 1->2->3->...->11 (no client 12). Should return false (no cycle found).
    assert!(!would_create_cycle(ClientId::new(0), ClientId::new(1), &clients));

    // Test with actual cycle at depth > 10
    // Make client 11 point back to client 1 (creates cycle 1->2->...->11->1)
    clients.get_mut(&ClientId::new(11)).unwrap().relation = Some(ClientRelation::Following {
        target: ClientId::new(1),
    });
    // would_create_cycle(0, 1, ...) traverses 1->2->...->10 (depth exhausted at 10)
    // It should return false because the safety limit prevents further traversal
    assert!(!would_create_cycle(ClientId::new(0), ClientId::new(1), &clients));
}
