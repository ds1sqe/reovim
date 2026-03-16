use super::*;

// ============================================================================
// BufferEntry
// ============================================================================

#[test]
fn buffer_entry_clone_eq() {
    let entry = BufferEntry {
        id: 1,
        name: String::from("main.rs"),
        path: Some(String::from("/src/main.rs")),
        modified: false,
        pinned: false,
        filetype: Some(String::from("rust")),
        error_count: 0,
        warning_count: 0,
    };
    let cloned = entry.clone();
    assert_eq!(entry, cloned);
}

#[test]
fn buffer_entry_debug() {
    let entry = BufferEntry {
        id: 1,
        name: String::from("test"),
        path: None,
        modified: true,
        pinned: false,
        filetype: None,
        error_count: 2,
        warning_count: 1,
    };
    let debug = format!("{entry:?}");
    assert!(debug.contains("test"));
    assert!(debug.contains("modified: true"));
}

// ============================================================================
// BufferlineSnapshot
// ============================================================================

#[test]
fn snapshot_create_empty() {
    let snap = BufferlineSnapshot::create();
    assert!(snap.entries.is_empty());
}

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
