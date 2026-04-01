use crate::api::BufferCapabilities;

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
fn intersection_works() {
    let common = BufferCapabilities::ROPE & BufferCapabilities::VIRTUAL;
    assert!(common.contains(BufferCapabilities::SNAPSHOTTABLE));
    assert!(common.contains(BufferCapabilities::LINE_READABLE));
    assert!(common.contains(BufferCapabilities::EDITABLE));
    assert!(!common.contains(BufferCapabilities::CONTENT_MATERIALIZABLE));
    assert!(!common.contains(BufferCapabilities::STREAMABLE));
}

#[test]
fn union_works() {
    let all = BufferCapabilities::ROPE | BufferCapabilities::VIRTUAL;
    assert!(all.contains(BufferCapabilities::CONTENT_MATERIALIZABLE));
    assert!(all.contains(BufferCapabilities::STREAMABLE));
    assert!(all.contains(BufferCapabilities::FILE_BACKED));
}

#[test]
fn empty_caps() {
    let empty = BufferCapabilities::empty();
    assert!(empty.is_empty());
    assert!(!empty.contains(BufferCapabilities::EDITABLE));
}

#[test]
fn bits_roundtrip() {
    let caps = BufferCapabilities::ROPE;
    let bits = caps.bits();
    let restored = BufferCapabilities::from_bits_truncate(bits);
    assert_eq!(caps, restored);
}
