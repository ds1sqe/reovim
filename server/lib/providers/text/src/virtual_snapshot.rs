//! Opaque snapshot of virtual buffer state.
//!
//! Moved from `reovim-kernel` to `reovim-provider-text` as part of #740
//! (kernel buffer extraction). `VirtualSnapshot` is text-specific: its
//! fields include `LineIndex` and byte-level `PieceTree` state.

use std::{fmt, sync::Arc};

use {
    reovim_domain_text::LineIndex,
    reovim_driver_vfs::{FileMapping, PieceTree},
};

/// Opaque snapshot of virtual buffer state.
///
/// Produced by [`VirtualBuffer::capture_snapshot`], consumed by
/// [`VirtualBuffer::restore_snapshot`].
#[derive(Clone)]
pub struct VirtualSnapshot {
    pub(crate) pieces: PieceTree,
    pub(crate) add_buffer_len: usize,
    pub(crate) original: Arc<dyn FileMapping>,
    pub(crate) line_index: LineIndex,
    pub(crate) crlf: bool,
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
