//! Tests for `projector.rs` — `TextProjector` coverage.
//!
//! Registered under the `selftest` feature; runs on the arch no_std selftest
//! runner via the `kernel-selftest` fixture bin.

use {
    reovim_arch::{arch_test, ds::Bytes},
    reovim_subsys_domain::{
        contract::RenderProjector as _,
        id::{BufferId, WindowId},
    },
};

use crate::projector::TextProjector;

arch_test!(projector_render_empty_buffer, {
    let p = TextProjector;
    let buf = Bytes::new();
    let proj = p
        .render(buf, 0, BufferId::new(1), WindowId::new(1))
        .expect("render empty buffer");
    assert!(proj.content.is_empty());
    assert_eq!(proj.span.start, 0);
    assert_eq!(proj.span.end, 0);
    assert_eq!(proj.cursor_byte, 0);
    assert_eq!(proj.buffer_id.as_u32(), 1);
    assert_eq!(proj.window_id.as_u32(), 1);
});

arch_test!(projector_render_non_empty_buffer, {
    let p = TextProjector;
    let buf = Bytes::try_from_slice(b"hello").expect("alloc");
    let proj = p
        .render(buf, 3, BufferId::new(2), WindowId::new(5))
        .expect("render non-empty buffer");
    assert_eq!(proj.content.as_slice(), b"hello");
    assert_eq!(proj.span.start, 0);
    assert_eq!(proj.span.end, 5);
    assert_eq!(proj.cursor_byte, 3);
    assert_eq!(proj.buffer_id.as_u32(), 2);
    assert_eq!(proj.window_id.as_u32(), 5);
});

arch_test!(projector_span_covers_full_buffer, {
    let p = TextProjector;
    let content = b"reovim is fast";
    let buf = Bytes::try_from_slice(content).expect("alloc");
    let len = content.len();
    let proj = p
        .render(buf, 0, BufferId::new(1), WindowId::new(1))
        .expect("render");
    assert_eq!(proj.span.end, len, "span must cover the full buffer");
    assert_eq!(proj.span.start, 0);
});

arch_test!(projector_encode_decode_round_trip, {
    use reovim_subsys_domain::projection::Projection;

    let p = TextProjector;
    let buf = Bytes::try_from_slice(b"test").expect("alloc");
    let proj = p
        .render(buf, 1, BufferId::new(7), WindowId::new(3))
        .expect("render");

    let encoded = proj.encode().expect("encode");
    let decoded = Projection::decode(encoded.as_slice()).expect("decode");
    assert_eq!(decoded.content.as_slice(), b"test");
    assert_eq!(decoded.cursor_byte, 1);
    assert_eq!(decoded.buffer_id.as_u32(), 7);
    assert_eq!(decoded.window_id.as_u32(), 3);
});
