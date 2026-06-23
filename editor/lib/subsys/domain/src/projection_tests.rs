//! Tests for `projection.rs` — `Projection` + `ProjectionSpan` coverage.
//!
//! Registered under the `selftest` feature; runs on the arch no_std selftest
//! runner via the `editor-core-selftest` fixture bin.

use core::num::NonZeroU32;

use {reovim_arch::arch_test, reovim_lib_ds::Bytes};

use crate::{
    carrier::{CursorCarrier, CursorHeader, PositionCarrier, PositionHeader},
    id::{BufferId, ClientId, DomainId, WindowId},
    projection::{
        FullProjection, FullProjectionSpan, GlyphHint, OverlayBlob, Projection, ProjectionDelivery,
        ProjectionRange, ProjectionSlotKey, ProjectionSpan, StyleRef,
    },
    routing::ProjectorId,
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

arch_test!(opaque_style_and_glyph_refs_round_trip, {
    assert_eq!(StyleRef::new(10).as_u32(), 10);
    assert_eq!(GlyphHint::new(11).as_u32(), 11);
    let span =
        FullProjectionSpan::new(ProjectionRange::new(1, 4), StyleRef::new(10), GlyphHint::new(11));
    assert_eq!(span.range.start, 1);
    assert_eq!(span.range.end, 4);
    assert!(!span.range.is_empty());
});

arch_test!(projection_slot_key_tracks_projector_identity, {
    let key = ProjectionSlotKey::new(
        ClientId::new(1),
        BufferId::new(2),
        WindowId::new(3),
        ProjectorId::Render,
    );
    assert_eq!(key.client_id, ClientId::new(1));
    assert_eq!(key.projector_id, ProjectorId::Render);
});

arch_test!(full_projection_holds_spans_cursors_and_overlays, {
    let domain = DomainId::new(NonZeroU32::new(1).unwrap());
    let mut full = FullProjection::new(
        ClientId::new(1),
        BufferId::new(2),
        WindowId::new(3),
        PositionCarrier::new(PositionHeader::new(domain, 0, 0), Bytes::new()),
    );
    full.spans
        .try_push(FullProjectionSpan::new(
            ProjectionRange::new(0, 5),
            StyleRef::new(1),
            GlyphHint::new(2),
        ))
        .expect("alloc");
    full.cursors
        .try_push(CursorCarrier::new(
            CursorHeader::new(domain, 0, 0),
            Bytes::try_from_slice(b"cursor").expect("alloc"),
        ))
        .expect("alloc");
    full.overlays
        .try_push(OverlayBlob::new(
            ProjectorId::custom(9),
            Bytes::try_from_slice(b"overlay").expect("alloc"),
        ))
        .expect("alloc");
    assert_eq!(full.spans.as_slice()[0].range.end, 5);
    assert_eq!(full.cursors.as_slice()[0].content.as_slice(), b"cursor");
    assert_eq!(full.overlays.as_slice()[0].payload.as_slice(), b"overlay");
});

arch_test!(projection_delivery_marks_full_resend_boundaries, {
    assert!(ProjectionDelivery::FullResend.requires_full_resend());
    assert!(!ProjectionDelivery::DiffAllowed.requires_full_resend());
});
