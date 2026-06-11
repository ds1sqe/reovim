//! `TextProjector` — `Render` projector for the text Domain (#797).
//!
//! Emits a [`Projection`] with one `ProjectionSpan` covering the full buffer
//! byte range and a cursor marker (§5.3 walking-skeleton subset). Overlays are
//! absent (empty). This is the minimal realization demanded by the walking
//! skeleton; the full §5.3 vocabulary (style, glyph hints, cursor carriers,
//! cross-Domain composition) arrives in #797 Phase 4+.

use {
    reovim_arch::ds::Bytes,
    reovim_subsys_domain::{
        contract::RenderProjector,
        id::{BufferId, WindowId},
        projection::{Projection, ProjectionSpan},
    },
};

// ── TextProjector ─────────────────────────────────────────────────────────────

/// The `Render` projector for the text Domain (walking-skeleton).
///
/// Stateless: all per-session state arrives as arguments via the CC14
/// dispatch (§2.3).
///
/// ```rust,no_run
/// // no_run: requires arch allocator runtime.
/// use reovim_domain_text::projector::TextProjector;
/// use reovim_subsys_domain::contract::RenderProjector as _;
/// use reovim_subsys_domain::id::{BufferId, WindowId};
/// use reovim_arch::ds::Bytes;
///
/// let p = TextProjector;
/// let buf = Bytes::try_from_slice(b"hello").unwrap();
/// let proj = p.render(buf, 2, BufferId::new(1), WindowId::new(1)).unwrap();
/// assert_eq!(proj.cursor_byte, 2);
/// assert_eq!(proj.span.start, 0);
/// assert_eq!(proj.span.end, 5);
/// ```
pub struct TextProjector;

impl RenderProjector for TextProjector {
    fn render(
        &self,
        buffer: Bytes,
        cursor: usize,
        buffer_id: BufferId,
        window_id: WindowId,
    ) -> Result<Projection, &'static str> {
        let end = buffer.len();
        // Clone buffer into the Projection's content field.
        let mut content = Bytes::new();
        content
            .try_extend_from_slice(buffer.as_slice())
            .map_err(|_| "alloc")?;
        Ok(Projection {
            buffer_id,
            window_id,
            span: ProjectionSpan { start: 0, end },
            cursor_byte: cursor,
            content,
        })
    }
}

// L12 layout: tests in sibling projector_tests.rs, declared in lib.rs.
