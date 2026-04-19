use super::*;

// =========================================================================
// SurfaceDescriptor tests
// =========================================================================

#[test]
fn test_surface_descriptor_new_and_accessors() {
    let body = vec![0x01u8, 0x02, 0x03];
    let sd = SurfaceDescriptor::new(SurfaceDescriptor::KIND_CELL_GRID, body.clone());
    assert_eq!(sd.kind(), SurfaceDescriptor::KIND_CELL_GRID);
    assert_eq!(sd.body(), body.as_slice());
}

#[test]
fn test_surface_descriptor_empty_body() {
    let sd = SurfaceDescriptor::new(SurfaceDescriptor::KIND_PIXEL_BUFFER, vec![]);
    assert_eq!(sd.kind(), SurfaceDescriptor::KIND_PIXEL_BUFFER);
    assert!(sd.body().is_empty());
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_surface_descriptor_debug() {
    let sd = SurfaceDescriptor::new(SurfaceDescriptor::KIND_VR_SCENE, vec![0xAB]);
    let s = format!("{sd:?}");
    assert!(s.contains("SurfaceDescriptor"));
}

#[test]
fn test_surface_descriptor_clone_and_eq() {
    let a = SurfaceDescriptor::new(SurfaceDescriptor::KIND_VOLUMETRIC, vec![1, 2, 3]);
    let b = a.clone();
    assert_eq!(a, b);

    let c = SurfaceDescriptor::new(SurfaceDescriptor::KIND_VOLUMETRIC, vec![1, 2, 4]);
    assert_ne!(a, c);
}

#[test]
fn test_surface_descriptor_kind_constants() {
    assert_eq!(SurfaceDescriptor::KIND_CELL_GRID, 0x0001);
    assert_eq!(SurfaceDescriptor::KIND_PIXEL_BUFFER, 0x0002);
    assert_eq!(SurfaceDescriptor::KIND_VR_SCENE, 0x0003);
    assert_eq!(SurfaceDescriptor::KIND_VOLUMETRIC, 0x0004);
    assert_eq!(SurfaceDescriptor::KIND_NEURAL, 0x0005);
}

#[test]
fn test_client_id() {
    let id = ClientId::new(42);
    assert_eq!(id.as_usize(), 42);
    assert_eq!(id.to_string(), "client-42");
}

#[test]
fn test_key_sequence() {
    let mut seq = KeySequence::new();
    assert!(seq.is_empty());

    seq.push("d".to_string());
    seq.push("w".to_string());
    assert!(!seq.is_empty());
    assert_eq!(seq.as_string(), "dw");

    seq.clear();
    assert!(seq.is_empty());
}

// =========================================================================
// Additional KeySequence tests
// =========================================================================

#[test]
fn test_key_sequence_keys() {
    let mut seq = KeySequence::new();
    seq.push("d".to_string());
    seq.push("w".to_string());

    let keys = seq.keys();
    assert_eq!(keys.len(), 2);
    assert_eq!(keys[0], "d");
    assert_eq!(keys[1], "w");
}

#[test]
fn test_key_sequence_default() {
    let seq = KeySequence::default();
    assert!(seq.is_empty());
    assert!(seq.keys().is_empty());
    assert!(seq.as_string().is_empty());
}

#[test]
fn test_key_sequence_as_string_single() {
    let mut seq = KeySequence::new();
    seq.push("a".to_string());
    assert_eq!(seq.as_string(), "a");
}

#[test]
fn test_key_sequence_special_keys() {
    let mut seq = KeySequence::new();
    seq.push("<C-w>".to_string());
    seq.push("h".to_string());
    assert_eq!(seq.as_string(), "<C-w>h");
    assert_eq!(seq.keys().len(), 2);
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_key_sequence_debug() {
    let seq = KeySequence::new();
    let debug = format!("{seq:?}");
    assert!(debug.contains("KeySequence"));
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
