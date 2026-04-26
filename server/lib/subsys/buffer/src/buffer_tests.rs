//! Compile-time shape tests for `Buffer` and `CodecAttachmentId`.

use {super::*, std::num::NonZeroU32};

// ── Object-safety / Send+Sync assertions ────────────────────────────────────

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn buffer_is_object_safe() {
    fn accepts_ref(_: &dyn Buffer) {}
    fn accepts_arc(_: std::sync::Arc<dyn Buffer>) {}
    let _: fn(&dyn Buffer) = accepts_ref;
    let _: fn(std::sync::Arc<dyn Buffer>) = accepts_arc;
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn buffer_trait_object_is_send_sync() {
    fn assert<T: Send + Sync + ?Sized>() {}
    assert::<dyn Buffer>();
}

// ── CodecAttachmentId ────────────────────────────────────────────────────────

#[test]
fn codec_attachment_id_hash_eq_round_trip() {
    use std::collections::HashMap;

    let id1 = CodecAttachmentId(NonZeroU32::new(1).unwrap());
    let id2 = CodecAttachmentId(NonZeroU32::new(2).unwrap());
    let id1b = CodecAttachmentId(NonZeroU32::new(1).unwrap());

    assert_eq!(id1, id1b);
    assert_ne!(id1, id2);

    let mut map: HashMap<CodecAttachmentId, &str> = HashMap::new();
    map.insert(id1, "first");
    map.insert(id2, "second");
    assert_eq!(map[&id1b], "first");
    assert_eq!(map[&id2], "second");
}

#[test]
fn codec_attachment_id_ordering() {
    let a = CodecAttachmentId(NonZeroU32::new(1).unwrap());
    let b = CodecAttachmentId(NonZeroU32::new(2).unwrap());
    assert!(a < b);
    assert!(b > a);

    let mut ids = [b, a];
    ids.sort();
    assert_eq!(ids[0], a);
    assert_eq!(ids[1], b);
}

#[test]
fn codec_attachment_id_copy() {
    let id = CodecAttachmentId(NonZeroU32::new(42).unwrap());
    let id2 = id;
    assert_eq!(id, id2);
}

#[test]
fn codec_attachment_id_debug() {
    let id = CodecAttachmentId(NonZeroU32::new(7).unwrap());
    let s = format!("{id:?}");
    assert!(s.contains('7'), "debug should contain the inner value");
}
