//! Opaque snapshot of virtual buffer state.
//!
//! [`VirtualSnapshot`] captures the internal state of a `VirtualBuffer`
//! (mmap + piece table) for snapshot/restore operations.  The type stays
//! in the kernel because its fields are all kernel types (`PieceTree`,
//! `FileMapping`, `LineIndex`).
//!
//! The [`SnapshotCapture`] trait abstracts concrete buffer types so that
//! `block/snapshot.rs` can capture/restore virtual snapshots without
//! depending on the concrete `VirtualBuffer` (which lives in
//! `reovim-provider-text`).

use std::{fmt, sync::Arc};

use super::{BufferId, file_mapping::FileMapping, line_index::LineIndex, piece_table::PieceTree};

// ─── VirtualSnapshot ────────────────────────────────────────────────────────

/// Opaque snapshot of virtual buffer state.
///
/// Produced by [`SnapshotCapture::capture_snapshot`], consumed by
/// [`SnapshotCapture::restore_snapshot`].  The `block/snapshot.rs` module
/// stores this type without knowing concrete buffer internals.
#[derive(Clone)]
pub struct VirtualSnapshot {
    pieces: PieceTree,
    add_buffer_len: usize,
    original: Arc<dyn FileMapping>,
    line_index: LineIndex,
    crlf: bool,
}

impl VirtualSnapshot {
    /// Create a new snapshot from its components.
    #[must_use]
    pub fn new(
        pieces: PieceTree,
        add_buffer_len: usize,
        original: Arc<dyn FileMapping>,
        line_index: LineIndex,
        crlf: bool,
    ) -> Self {
        Self {
            pieces,
            add_buffer_len,
            original,
            line_index,
            crlf,
        }
    }

    /// Length of the add buffer at snapshot time.
    #[must_use]
    pub const fn add_buffer_len(&self) -> usize {
        self.add_buffer_len
    }

    /// Access the captured piece tree.
    #[must_use]
    pub const fn pieces(&self) -> &PieceTree {
        &self.pieces
    }

    /// Access the captured original file mapping.
    #[must_use]
    pub fn original(&self) -> &Arc<dyn FileMapping> {
        &self.original
    }

    /// Access the captured line index.
    #[must_use]
    pub const fn line_index(&self) -> &LineIndex {
        &self.line_index
    }

    /// Whether the captured content used CRLF line endings.
    #[must_use]
    pub const fn crlf(&self) -> bool {
        self.crlf
    }

    /// Destructure into components (for restore operations).
    #[must_use]
    pub fn into_parts(self) -> (PieceTree, usize, Arc<dyn FileMapping>, LineIndex, bool) {
        (self.pieces, self.add_buffer_len, self.original, self.line_index, self.crlf)
    }
}

impl fmt::Debug for VirtualSnapshot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("VirtualSnapshot")
            .field("piece_count", &self.pieces.piece_count())
            .field("add_buffer_len", &self.add_buffer_len)
            .field("original_len", &self.original.len())
            .field("line_count", &self.line_index.line_count())
            .field("crlf", &self.crlf)
            .finish()
    }
}

// ─── SnapshotCapture ────────────────────────────────────────────────────────

/// Trait for buffer types that support snapshot capture/restore.
///
/// This abstracts `VirtualBuffer` so that `block/snapshot.rs` can capture
/// and restore virtual snapshots without depending on the concrete type
/// (which lives in `reovim-provider-text`).
///
/// # Implementors
///
/// - `VirtualBuffer` (in `reovim-provider-text`)
pub trait SnapshotCapture: Send + Sync {
    /// Capture an opaque snapshot of this buffer's state.
    fn capture_snapshot(&self) -> VirtualSnapshot;

    /// Restore state from a snapshot.
    fn restore_snapshot(&mut self, snap: VirtualSnapshot);

    /// Get the buffer ID for snapshot matching.
    fn snapshot_buffer_id(&self) -> BufferId;
}

// ─── RopeCapture ────────────────────────────────────────────────────────────

use super::rope::Rope;

/// Trait for Rope-based buffer types that support snapshot capture/restore.
///
/// This abstracts `Buffer` (which lives in the kernel during the transition
/// and will move to `reovim-provider-text`) so that `block/snapshot.rs`
/// can capture and restore rope-based snapshots without a concrete dependency
/// on the `Buffer` type.
///
/// # Implementors
///
/// - `Buffer` (currently in `reovim-kernel`, migrating to `reovim-provider-text`)
pub trait RopeCapture: Send + Sync {
    /// Capture the current rope (O(1) clone via Arc sharing).
    fn capture_rope(&self) -> Rope;

    /// Restore state from a rope snapshot.
    fn restore_rope(&mut self, rope: Rope);

    /// Get the buffer ID for snapshot matching.
    fn rope_buffer_id(&self) -> BufferId;
}
