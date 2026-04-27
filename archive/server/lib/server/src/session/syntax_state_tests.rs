use super::*;

// ========================================================================
// SyntaxStreamState tests (server-owned infrastructure)
// ========================================================================

#[test]
fn test_stream_state_new() {
    let state = SyntaxStreamState::new();
    assert_eq!(state.subscriber_count(), 0);
    assert!(!state.has_subscribers());
}

#[test]
fn test_stream_state_session_extension() {
    let state = SyntaxStreamState::create();
    assert_eq!(state.subscriber_count(), 0);
}

#[test]
fn test_subscribe() {
    let mut state = SyntaxStreamState::new();
    assert_eq!(state.subscriber_count(), 0);

    let _rx1 = state.subscribe();
    assert_eq!(state.subscriber_count(), 1);
    assert!(state.has_subscribers());

    let _rx2 = state.subscribe();
    assert_eq!(state.subscriber_count(), 2);
}

#[test]
fn test_broadcast() {
    let mut state = SyntaxStreamState::new();
    let mut rx = state.subscribe();

    let update = TokenUpdate { buffer_id: 1 };

    state.broadcast(&update);

    let received = rx.try_recv().expect("Should receive update");
    assert_eq!(received.buffer_id, 1);
}

#[test]
fn test_broadcast_removes_disconnected() {
    let mut state = SyntaxStreamState::new();
    let rx = state.subscribe();
    assert_eq!(state.subscriber_count(), 1);

    drop(rx);

    let update = TokenUpdate { buffer_id: 1 };

    state.broadcast(&update);

    assert_eq!(state.subscriber_count(), 0);
}

#[test]
fn test_debug_impl() {
    let mut state = SyntaxStreamState::new();
    let _rx = state.subscribe();

    let debug = format!("{state:?}");
    assert!(debug.contains("SyntaxStreamState"));
    assert!(debug.contains("subscriber_count"));
}

// SyntaxSessionState tests: REMOVED (#753 E6).
// SyntaxSessionState is domain-owned — tests live in driver-text-syntax.
// notify_edit, send_full_refresh, build_token_update: REMOVED (#753 E6).
// text_event_to_syntax_edit, compute_end_position: REMOVED (#753 E6).
// These functions are in driver-text-syntax, not the server crate.
