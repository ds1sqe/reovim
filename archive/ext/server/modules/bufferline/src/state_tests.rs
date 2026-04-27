use super::*;

// ============================================================================
// BufferlineState
// ============================================================================

#[test]
fn state_create_empty() {
    let state = BufferlineState::create();
    assert!(state.pinned.is_empty());
}

#[test]
fn state_pin_adds() {
    let mut state = BufferlineState::create();
    state.pin(1);
    assert!(state.is_pinned(1));
    assert_eq!(state.pinned.len(), 1);
}

#[test]
fn state_pin_duplicate_noop() {
    let mut state = BufferlineState::create();
    state.pin(1);
    state.pin(1);
    assert_eq!(state.pinned.len(), 1);
}

#[test]
fn state_unpin_removes() {
    let mut state = BufferlineState::create();
    state.pin(1);
    state.pin(2);
    state.unpin(1);
    assert!(!state.is_pinned(1));
    assert!(state.is_pinned(2));
}

#[test]
fn state_unpin_nonexistent_noop() {
    let mut state = BufferlineState::create();
    state.unpin(99);
    assert!(state.pinned.is_empty());
}

#[test]
fn state_is_pinned() {
    let mut state = BufferlineState::create();
    assert!(!state.is_pinned(1));
    state.pin(1);
    assert!(state.is_pinned(1));
    assert!(!state.is_pinned(2));
}

#[test]
fn state_clean_stale() {
    let mut state = BufferlineState::create();
    state.pin(1);
    state.pin(2);
    state.pin(3);
    state.clean_stale(&[1, 3]);
    assert!(state.is_pinned(1));
    assert!(!state.is_pinned(2));
    assert!(state.is_pinned(3));
}

#[test]
fn state_clean_stale_empty_live() {
    let mut state = BufferlineState::create();
    state.pin(1);
    state.clean_stale(&[]);
    assert!(state.pinned.is_empty());
}
