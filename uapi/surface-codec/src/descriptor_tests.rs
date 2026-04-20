//! Tests for `SurfaceDescriptor`, header accessors, and `SurfacePayloadError`.

use {
    super::{
        Codec, KIND_CELL_GRID, KIND_PIXEL_BUFFER, SurfaceDescriptor, SurfacePayloadError,
        surface_body, surface_kind,
    },
    std::any::Any,
};

#[test]
fn new_stores_kind_and_body() {
    let d = SurfaceDescriptor::new(0x0001, vec![1, 2, 3]);
    assert_eq!(d.kind, 0x0001);
    assert_eq!(d.body, vec![1, 2, 3]);
}

#[test]
fn kind_accessor_matches_field() {
    let d = SurfaceDescriptor::new(0x0002, Vec::new());
    assert_eq!(d.kind(), 0x0002);
}

#[test]
fn body_accessor_matches_field() {
    let d = SurfaceDescriptor::new(0x0001, vec![4, 5, 6]);
    assert_eq!(d.body(), &[4, 5, 6]);
}

#[test]
fn surface_kind_header_accessor() {
    let d = SurfaceDescriptor::new(0x0042, Vec::new());
    assert_eq!(surface_kind(&d), 0x0042);
}

#[test]
fn surface_body_header_accessor() {
    let d = SurfaceDescriptor::new(0x0001, vec![7, 8, 9]);
    assert_eq!(surface_body(&d), &[7, 8, 9]);
}

#[test]
fn well_known_kinds_are_distinct() {
    let kinds = [KIND_CELL_GRID, KIND_PIXEL_BUFFER];
    for (i, a) in kinds.iter().enumerate() {
        for b in &kinds[(i + 1)..] {
            assert_ne!(a, b, "well-known surface kinds must be distinct");
        }
    }
}

#[test]
fn well_known_kinds_in_reserved_range() {
    for kind in [KIND_CELL_GRID, KIND_PIXEL_BUFFER] {
        assert!(
            (0x0001..=0x00FF).contains(&kind),
            "in-repo kinds must live in 0x0001..=0x00FF (got {kind:#06x})"
        );
    }
}

#[test]
fn descriptor_clone_and_eq() {
    let d = SurfaceDescriptor::new(0x0001, vec![1, 2]);
    let c = d.clone();
    assert_eq!(d, c);
    let e = SurfaceDescriptor::new(0x0002, vec![1, 2]);
    assert_ne!(d, e);
    let f = SurfaceDescriptor::new(0x0001, vec![1, 3]);
    assert_ne!(d, f);
}

#[test]
fn descriptor_debug_contains_kind() {
    let d = SurfaceDescriptor::new(0x0001, vec![1]);
    let s = format!("{d:?}");
    assert!(s.contains("SurfaceDescriptor"));
    assert!(s.contains("kind"));
}

// ── SurfacePayloadError ─────────────────────────────────────────────────────

#[test]
fn payload_error_too_short_display() {
    let e = SurfacePayloadError::TooShort { got: 2, min: 4 };
    let s = format!("{e}");
    assert!(s.contains('2'));
    assert!(s.contains('4'));
}

#[test]
fn payload_error_wrong_type_display() {
    let e = SurfacePayloadError::WrongType {
        expected: 0x0001,
        actual: 0x0002,
    };
    let s = format!("{e}");
    assert!(s.contains("0x0001"));
    assert!(s.contains("0x0002"));
}

#[test]
fn payload_error_invalid_data_display() {
    let e = SurfacePayloadError::InvalidData {
        reason: "bad magic",
    };
    let s = format!("{e}");
    assert!(s.contains("bad magic"));
}

#[test]
fn payload_error_is_error() {
    let e: Box<dyn std::error::Error> = Box::new(SurfacePayloadError::TooShort { got: 0, min: 1 });
    assert!(!e.to_string().is_empty());
}

// ── Codec trait object-safety smoke ─────────────────────────────────────────

struct StubSurfaceCodec {
    kind: u16,
}

impl Codec for StubSurfaceCodec {
    fn kind(&self) -> u16 {
        self.kind
    }

    fn encode(&self, _value: &dyn Any) -> Result<Vec<u8>, SurfacePayloadError> {
        Ok(vec![0; 4])
    }

    fn decode(&self, _body: &[u8]) -> Result<Box<dyn Any + Send>, SurfacePayloadError> {
        Ok(Box::new(()))
    }
}

#[test]
fn codec_is_object_safe() {
    let codec: Box<dyn Codec> = Box::new(StubSurfaceCodec { kind: 0x0001 });
    assert_eq!(codec.kind(), 0x0001);
    let encoded = codec.encode(&()).unwrap();
    assert_eq!(encoded, vec![0; 4]);
    let decoded = codec.decode(&encoded).unwrap();
    let _: &dyn Any = decoded.as_ref();
}
