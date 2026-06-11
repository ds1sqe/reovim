//! Tests for `projection.rs` — `Projection` + `ProjectionSpan` coverage.
//!
//! Registered under the `selftest` feature; runs on the arch no_std selftest
//! runner via the `kernel-selftest` fixture bin.

use reovim_arch::{arch_test, ds::Bytes};

use crate::{
    id::{BufferId, WindowId},
    projection::{Projection, ProjectionSpan},
};

arch_test!(projection_span_fields_round_trip, {
    let span = ProjectionSpan { start: 3, end: 10 };
    assert_eq!(span.start, 3);
    assert_eq!(span.end, 10);
    assert_eq!(span.end - span.start, 7);
});

arch_test!(projection_encode_decode_round_trip, {
    let mut content = Bytes::new();
    content.try_extend_from_slice(b"hello").expect("extend");

    let proj = Projection {
        buffer_id: BufferId::new(2),
        window_id: WindowId::new(3),
        span: ProjectionSpan { start: 0, end: 5 },
        cursor_byte: 2,
        content,
    };

    let encoded = proj.encode().expect("encode succeeds");
    // Encoded = 4 (bid) + 4 (wid) + 4 (cursor) + 4 (span_end) + 5 (content) = 21 bytes.
    assert_eq!(encoded.len(), 21);

    let decoded = Projection::decode(encoded.as_slice()).expect("decode succeeds");
    assert_eq!(decoded.buffer_id.as_u32(), 2);
    assert_eq!(decoded.window_id.as_u32(), 3);
    assert_eq!(decoded.cursor_byte, 2);
    assert_eq!(decoded.span.start, 0);
    assert_eq!(decoded.span.end, 5);
    assert_eq!(decoded.content.as_slice(), b"hello");
});

arch_test!(projection_decode_truncated_returns_err, {
    let too_short = b"\x01\x00\x00\x00\x01\x00\x00\x00\x00\x00\x00";
    let result = Projection::decode(too_short);
    assert!(result.is_err(), "decode of truncated bytes returns Err");
});

arch_test!(projection_encode_empty_content, {
    let proj = Projection {
        buffer_id: BufferId::new(1),
        window_id: WindowId::new(1),
        span: ProjectionSpan { start: 0, end: 0 },
        cursor_byte: 0,
        content: Bytes::new(),
    };
    let encoded = proj.encode().expect("encode empty content");
    assert_eq!(encoded.len(), 16, "header only with no content");
    let decoded = Projection::decode(encoded.as_slice()).expect("decode empty");
    assert!(decoded.content.is_empty());
    assert_eq!(decoded.cursor_byte, 0);
});
