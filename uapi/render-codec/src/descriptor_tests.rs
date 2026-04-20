//! Tests for `RenderDescriptor`, header accessors, and `RenderPayloadError`.

use {
    super::{
        Codec, KIND_CELL_GRID, RenderDescriptor, RenderPayloadError, render_body, render_kind,
    },
    std::any::Any,
};

#[test]
fn new_stores_kind_and_body() {
    let d = RenderDescriptor::new(0x0001, vec![1, 2, 3]);
    assert_eq!(d.kind, 0x0001);
    assert_eq!(d.body, vec![1, 2, 3]);
}

#[test]
fn kind_accessor_matches_field() {
    let d = RenderDescriptor::new(0x0002, Vec::new());
    assert_eq!(d.kind(), 0x0002);
}

#[test]
fn body_accessor_matches_field() {
    let d = RenderDescriptor::new(0x0001, vec![4, 5, 6]);
    assert_eq!(d.body(), &[4, 5, 6]);
}

#[test]
fn render_kind_header_accessor() {
    let d = RenderDescriptor::new(0x0042, Vec::new());
    assert_eq!(render_kind(&d), 0x0042);
}

#[test]
fn render_body_header_accessor() {
    let d = RenderDescriptor::new(0x0001, vec![7, 8, 9]);
    assert_eq!(render_body(&d), &[7, 8, 9]);
}

#[test]
fn well_known_kind_in_reserved_range() {
    assert!(
        (0x0001..=0x00FF).contains(&KIND_CELL_GRID),
        "in-repo kinds must live in 0x0001..=0x00FF (got {KIND_CELL_GRID:#06x})"
    );
}

#[test]
fn descriptor_clone_and_eq() {
    let d = RenderDescriptor::new(0x0001, vec![1, 2]);
    let c = d.clone();
    assert_eq!(d, c);
    let e = RenderDescriptor::new(0x0002, vec![1, 2]);
    assert_ne!(d, e);
    let f = RenderDescriptor::new(0x0001, vec![1, 3]);
    assert_ne!(d, f);
}

#[test]
fn descriptor_debug_contains_kind() {
    let d = RenderDescriptor::new(0x0001, vec![1]);
    let s = format!("{d:?}");
    assert!(s.contains("RenderDescriptor"));
    assert!(s.contains("kind"));
}

// ── RenderPayloadError ──────────────────────────────────────────────────────

#[test]
fn payload_error_too_short_display() {
    let e = RenderPayloadError::TooShort { got: 2, min: 4 };
    let s = format!("{e}");
    assert!(s.contains('2'));
    assert!(s.contains('4'));
}

#[test]
fn payload_error_wrong_type_display() {
    let e = RenderPayloadError::WrongType {
        expected: 0x0001,
        actual: 0x0002,
    };
    let s = format!("{e}");
    assert!(s.contains("0x0001"));
    assert!(s.contains("0x0002"));
}

#[test]
fn payload_error_invalid_data_display() {
    let e = RenderPayloadError::InvalidData {
        reason: "bad magic",
    };
    let s = format!("{e}");
    assert!(s.contains("bad magic"));
}

#[test]
fn payload_error_is_error() {
    let e: Box<dyn std::error::Error> = Box::new(RenderPayloadError::TooShort { got: 0, min: 1 });
    assert!(!e.to_string().is_empty());
}

// ── Codec trait object-safety smoke ─────────────────────────────────────────

struct StubRenderCodec {
    kind: u16,
}

impl Codec for StubRenderCodec {
    fn kind(&self) -> u16 {
        self.kind
    }

    fn encode(&self, _value: &dyn Any) -> Result<Vec<u8>, RenderPayloadError> {
        Ok(vec![0; 4])
    }

    fn decode(&self, _body: &[u8]) -> Result<Box<dyn Any + Send>, RenderPayloadError> {
        Ok(Box::new(()))
    }
}

#[test]
fn codec_is_object_safe() {
    let codec: Box<dyn Codec> = Box::new(StubRenderCodec { kind: 0x0001 });
    assert_eq!(codec.kind(), 0x0001);
    let encoded = codec.encode(&()).unwrap();
    assert_eq!(encoded, vec![0; 4]);
    let decoded = codec.decode(&encoded).unwrap();
    let _: &dyn Any = decoded.as_ref();
}
