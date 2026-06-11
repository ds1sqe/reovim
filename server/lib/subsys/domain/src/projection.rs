//! `Projection` and `ProjectionSpan` — domain-neutral view of buffer content
//! (§5.3 walking-skeleton subset, #797).
//!
//! The walking skeleton realizes the **minimal `Projection` subset** (§5.3
//! subset note): one `ProjectionSpan` over the full buffer byte range plus a
//! cursor marker. Overlays are present but empty. The §1 full struct is the
//! spec target; this module grows toward it monotonically.
//!
//! ## Wire encoding
//!
//! The runtime (Phase 3) encodes a `Projection` into the
//! `AttachEvent::Projection` body (`projection: bytes`). The encoding format
//! used by the skeleton is a flat byte layout (not ABI-frozen; the frozen
//! encoding arrives with the content-codec feature):
//!
//! ```text
//! [0..4]   buffer_id (u32 LE)
//! [4..8]   window_id (u32 LE)
//! [8..12]  cursor_byte (u32 LE)
//! [12..16] span_end (u32 LE)  — buffer byte length
//! [16..]   buffer bytes (raw UTF-8)
//! ```

use reovim_arch::ds::Bytes;

use crate::id::{BufferId, WindowId};

// ── ProjectionSpan ────────────────────────────────────────────────────────────

/// One styled byte range in a `Projection` (§5.3 §1).
///
/// The walking-skeleton subset carries only `range` (start + end byte offsets).
/// `style` and `glyph` are deferred (opaque § 8.3 vocabulary not yet concrete).
///
/// ```rust
/// use reovim_subsys_domain::projection::ProjectionSpan;
///
/// let span = ProjectionSpan { start: 0, end: 5 };
/// assert_eq!(span.end - span.start, 5);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProjectionSpan {
    /// Inclusive start byte in the buffer.
    pub start: usize,
    /// Exclusive end byte in the buffer.
    pub end: usize,
}

// ── Projection ────────────────────────────────────────────────────────────────

/// Domain-neutral view of buffer content produced by the `Render` projector
/// (§5.3 §1, walking-skeleton subset).
///
/// The kernel produces a `Projection` after each `OnRawInput` → `Render`
/// dispatch cycle. The runtime (Phase 3) encodes it into the wire representation
/// and pushes it to connected clients via `AttachEvent::Projection`.
///
/// Fields present in the skeleton:
/// - `buffer_id`, `window_id` — routing identifiers.
/// - `spans` — one span over the full buffer byte range.
/// - `cursor_byte` — cursor position in bytes.
/// - `content` — the buffer bytes at projection time.
///
/// Fields deferred to Phase 4: cursors/selections carriers, overlays,
/// cross-Domain composition (§5.3 §4).
///
/// ```rust,no_run
/// // no_run: requires arch allocator runtime (Bytes::new() uses the allocator).
/// ```
pub struct Projection {
    /// The buffer this projection covers.
    pub buffer_id: BufferId,
    /// The window this projection is for.
    pub window_id: WindowId,
    /// One span over the buffer byte range (walking-skeleton: full range).
    pub span: ProjectionSpan,
    /// Cursor position in bytes within the buffer.
    pub cursor_byte: usize,
    /// Buffer content at the time of projection.
    pub content: Bytes,
}

/// Why a [`Projection::decode`] call was refused.
///
/// # Example
///
/// ```rust
/// use reovim_subsys_domain::projection::ProjectionDecodeError;
///
/// assert_ne!(
///     ProjectionDecodeError::Truncated,
///     ProjectionDecodeError::Alloc,
/// );
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectionDecodeError {
    /// `bytes` is shorter than the 16-byte header.
    Truncated,
    /// The backing allocation for the decoded content failed.
    Alloc,
}

impl Projection {
    /// Encodes the projection into a flat byte representation suitable for
    /// transmission as `AttachEvent::Projection.projection` bytes.
    ///
    /// Layout:
    /// ```text
    /// [0..4]   buffer_id (u32 LE)
    /// [4..8]   window_id (u32 LE)
    /// [8..12]  cursor_byte (u32 LE)
    /// [12..16] span_end (u32 LE, = buffer byte length)
    /// [16..]   buffer bytes
    /// ```
    ///
    /// # Errors
    ///
    /// Returns `Err("alloc")` when the backing allocation for the encoded bytes
    /// fails.
    ///
    /// ```rust,no_run
    /// // no_run: requires arch allocator runtime.
    /// ```
    pub fn encode(&self) -> Result<Bytes, &'static str> {
        let mut out = Bytes::new();
        // buffer_id LE u32
        let bid = self.buffer_id.as_u32().to_le_bytes();
        out.try_extend_from_slice(&bid).map_err(|_| "alloc")?;
        // window_id LE u32
        let wid = self.window_id.as_u32().to_le_bytes();
        out.try_extend_from_slice(&wid).map_err(|_| "alloc")?;
        // cursor_byte LE u32 (truncate to u32 range — adequate for the skeleton)
        #[allow(clippy::cast_possible_truncation)]
        let cur = (self.cursor_byte as u32).to_le_bytes();
        out.try_extend_from_slice(&cur).map_err(|_| "alloc")?;
        // span_end LE u32
        #[allow(clippy::cast_possible_truncation)]
        let end = (self.span.end as u32).to_le_bytes();
        out.try_extend_from_slice(&end).map_err(|_| "alloc")?;
        // buffer bytes
        out.try_extend_from_slice(self.content.as_slice())
            .map_err(|_| "alloc")?;
        Ok(out)
    }

    /// Decodes a `Projection` from the flat byte representation produced by
    /// [`encode`](Self::encode).
    ///
    /// # Errors
    ///
    /// Returns [`ProjectionDecodeError::Truncated`] if `bytes` is shorter
    /// than the 16-byte header, and [`ProjectionDecodeError::Alloc`] if the
    /// backing allocation for `content` fails.
    ///
    /// ```rust,no_run
    /// // no_run: requires arch allocator runtime.
    /// ```
    pub fn decode(bytes: &[u8]) -> Result<Self, ProjectionDecodeError> {
        // Checked little-endian u32 read; `None` only when the slice is short.
        let le_u32 = |at: usize| -> Option<u32> {
            let chunk: &[u8; 4] = bytes.get(at..at + 4)?.try_into().ok()?;
            Some(u32::from_le_bytes(*chunk))
        };
        let header = (le_u32(0), le_u32(4), le_u32(8), le_u32(12));
        let (Some(buf), Some(win), Some(cursor), Some(end)) = header else {
            return Err(ProjectionDecodeError::Truncated);
        };
        let buffer_id = BufferId::new(buf);
        let window_id = WindowId::new(win);
        let cursor_byte = cursor as usize;
        let span_end = end as usize;
        let content_bytes = &bytes[16..];
        let mut content = Bytes::new();
        content
            .try_extend_from_slice(content_bytes)
            .map_err(|_| ProjectionDecodeError::Alloc)?;
        Ok(Self {
            buffer_id,
            window_id,
            span: ProjectionSpan {
                start: 0,
                end: span_end,
            },
            cursor_byte,
            content,
        })
    }
}

// L12 layout: tests in sibling projection_tests.rs, declared in lib.rs.
