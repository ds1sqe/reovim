use super::*;

#[test]
fn test_client_id() {
    let id = ClientId::new(42);
    assert_eq!(id.as_usize(), 42);
    assert_eq!(id.to_string(), "client-42");
}

// =========================================================================
// ClientId tests
// =========================================================================

#[test]
fn test_client_id_hash() {
    use std::collections::HashSet;

    let mut set = HashSet::new();
    set.insert(ClientId::new(1));
    set.insert(ClientId::new(2));
    set.insert(ClientId::new(1)); // duplicate

    assert_eq!(set.len(), 2);
}

#[test]
fn test_client_id_clone_copy() {
    let id = ClientId::new(42);
    let cloned = id;
    assert_eq!(id, cloned);
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_client_id_debug() {
    let id = ClientId::new(42);
    let debug = format!("{id:?}");
    assert!(debug.contains("42"));
}

#[test]
fn test_client_id_display_format() {
    assert_eq!(ClientId::new(0).to_string(), "client-0");
    assert_eq!(ClientId::new(100).to_string(), "client-100");
}

// =========================================================================
// CursorSnapshot tests
// =========================================================================

#[test]
fn test_cursor_snapshot_from_bytes() {
    let bytes = [1u8, 2, 3, 4, 5, 6, 7, 8];
    let snap = CursorSnapshot::from_bytes(bytes);
    assert_eq!(snap.as_bytes(), &bytes);
}

#[test]
fn test_cursor_snapshot_sentinel() {
    let snap = CursorSnapshot::create();
    assert_eq!(snap, CursorSnapshot::SENTINEL);
    assert_eq!(snap.as_bytes(), &[0u8; 8]);
}

#[test]
fn test_cursor_snapshot_eq() {
    let a = CursorSnapshot::from_bytes([1, 2, 3, 4, 5, 6, 7, 8]);
    let b = CursorSnapshot::from_bytes([1, 2, 3, 4, 5, 6, 7, 8]);
    let c = CursorSnapshot::from_bytes([1, 2, 3, 4, 5, 6, 7, 9]);
    assert_eq!(a, b);
    assert_ne!(a, c);
}

#[test]
fn test_cursor_snapshot_clone_copy() {
    let snap = CursorSnapshot::from_bytes([10, 20, 30, 40, 50, 60, 70, 80]);
    let copied = snap;
    assert_eq!(snap, copied);
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_cursor_snapshot_debug() {
    let snap = CursorSnapshot::from_bytes([1, 2, 3, 4, 5, 6, 7, 8]);
    let debug = format!("{snap:?}");
    assert!(debug.contains("CursorSnapshot"));
}
