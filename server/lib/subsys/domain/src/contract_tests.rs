//! Tests for `contract.rs` — `OnRawInputHandler` / `RenderProjector` coverage.
//!
//! Registered under the `selftest` feature; runs on the arch no_std selftest
//! runner via the `kernel-selftest` fixture bin. The traits are abstract, so
//! the tests define minimal in-test implementations and exercise them through
//! the trait objects (the same `&dyn` routing path the kernel uses).

use reovim_arch::{arch_test, ds::Bytes};

use crate::{
    contract::{OnRawInputHandler, RawInputResult, RenderProjector},
    id::{BufferId, WindowId},
    projection::Projection,
};

/// Minimal handler: appends every input byte to the buffer, advancing cursor.
struct AppendHandler;

impl OnRawInputHandler for AppendHandler {
    fn on_raw_input(&self, buffer: Bytes, cursor: usize, input: &[u8]) -> RawInputResult {
        let mut out = buffer;
        for &b in input {
            out.try_push(b).expect("alloc");
        }
        RawInputResult::claimed(out, cursor + input.len())
    }
}

/// Minimal projector: emits a full-buffer-span projection.
struct FullSpanProjector;

impl RenderProjector for FullSpanProjector {
    fn render(
        &self,
        buffer: Bytes,
        cursor: usize,
        buffer_id: BufferId,
        window_id: WindowId,
    ) -> Result<Projection, &'static str> {
        let end = buffer.len();
        Ok(Projection {
            buffer_id,
            window_id,
            span: crate::projection::ProjectionSpan { start: 0, end },
            cursor_byte: cursor,
            content: buffer,
        })
    }
}

arch_test!(handler_trait_object_appends_bytes, {
    let h: &dyn OnRawInputHandler = &AppendHandler;
    let result = h.on_raw_input(Bytes::new(), 0, b"ab");
    assert_eq!(result.buffer.as_slice(), b"ab");
    assert_eq!(result.cursor, 2);
    assert!(result.claimed);
});

arch_test!(projector_trait_object_full_span, {
    let p: &dyn RenderProjector = &FullSpanProjector;
    let buf = Bytes::try_from_slice(b"hello").expect("alloc");
    let proj = p
        .render(buf, 2, BufferId::new(1), WindowId::new(1))
        .expect("render");
    assert_eq!(proj.content.as_slice(), b"hello");
    assert_eq!(proj.span.end, 5);
    assert_eq!(proj.cursor_byte, 2);
});
