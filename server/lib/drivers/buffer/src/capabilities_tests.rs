use super::*;

#[test]
fn rope_preset_has_expected_flags() {
    let caps = BufferCapabilities::ROPE;
    assert!(caps.contains(BufferCapabilities::CONTENT_MATERIALIZABLE));
    assert!(caps.contains(BufferCapabilities::SNAPSHOTTABLE));
    assert!(caps.contains(BufferCapabilities::LINE_READABLE));
    assert!(caps.contains(BufferCapabilities::EDITABLE));
    assert!(!caps.contains(BufferCapabilities::STREAMABLE));
    assert!(!caps.contains(BufferCapabilities::FILE_BACKED));
}

#[test]
fn virtual_preset_has_expected_flags() {
    let caps = BufferCapabilities::VIRTUAL;
    assert!(caps.contains(BufferCapabilities::SNAPSHOTTABLE));
    assert!(caps.contains(BufferCapabilities::STREAMABLE));
    assert!(caps.contains(BufferCapabilities::FILE_BACKED));
    assert!(caps.contains(BufferCapabilities::LINE_READABLE));
    assert!(caps.contains(BufferCapabilities::EDITABLE));
    assert!(!caps.contains(BufferCapabilities::CONTENT_MATERIALIZABLE));
}

#[test]
fn presets_differ() {
    assert_ne!(BufferCapabilities::ROPE, BufferCapabilities::VIRTUAL);
}

#[test]
fn bitwise_operations() {
    let caps = BufferCapabilities::LINE_READABLE | BufferCapabilities::EDITABLE;
    assert!(caps.contains(BufferCapabilities::LINE_READABLE));
    assert!(caps.contains(BufferCapabilities::EDITABLE));
    assert!(!caps.contains(BufferCapabilities::SNAPSHOTTABLE));
}

#[test]
fn intersection() {
    let common = BufferCapabilities::ROPE & BufferCapabilities::VIRTUAL;
    assert!(common.contains(BufferCapabilities::SNAPSHOTTABLE));
    assert!(common.contains(BufferCapabilities::LINE_READABLE));
    assert!(common.contains(BufferCapabilities::EDITABLE));
    assert!(!common.contains(BufferCapabilities::CONTENT_MATERIALIZABLE));
    assert!(!common.contains(BufferCapabilities::STREAMABLE));
}

#[test]
fn empty_capabilities() {
    let caps = BufferCapabilities::empty();
    assert!(caps.is_empty());
    assert!(!caps.contains(BufferCapabilities::LINE_READABLE));
}

#[test]
fn all_capabilities() {
    let caps = BufferCapabilities::all();
    assert!(caps.contains(BufferCapabilities::CONTENT_MATERIALIZABLE));
    assert!(caps.contains(BufferCapabilities::SNAPSHOTTABLE));
    assert!(caps.contains(BufferCapabilities::STREAMABLE));
    assert!(caps.contains(BufferCapabilities::FILE_BACKED));
    assert!(caps.contains(BufferCapabilities::LINE_READABLE));
    assert!(caps.contains(BufferCapabilities::EDITABLE));
}

#[test]
fn bits_roundtrip() {
    let caps = BufferCapabilities::ROPE;
    let bits = caps.bits();
    let restored = BufferCapabilities::from_bits_truncate(bits);
    assert_eq!(caps, restored);
}

#[test]
fn virtual_bits_roundtrip() {
    let caps = BufferCapabilities::VIRTUAL;
    let bits = caps.bits();
    let restored = BufferCapabilities::from_bits_truncate(bits);
    assert_eq!(caps, restored);
}
