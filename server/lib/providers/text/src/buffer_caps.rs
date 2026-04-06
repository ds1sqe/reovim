//! Buffer capability flags.
//!
//! `BufferCapabilities` describes what operations a text buffer supports.
//! This replaces boolean `is_virtual_buffer()` checks with fine-grained
//! capability queries, following the Unix capabilities model.
//!
//! # Architecture
//!
//! ```text
//! Provider (here):  BufferCapabilities definition (text-level capability flags)
//! Driver:           reovim-driver-buffer re-exports for backward compat
//! Server:           gRPC sends capabilities as u32 in BufferInfo proto
//! ```

use bitflags::bitflags;

bitflags! {
    /// Describes what operations a buffer supports.
    ///
    /// Each buffer type advertises its capabilities via these flags.
    /// Callers query capabilities instead of checking concrete types.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct BufferCapabilities: u32 {
        /// Full content can be materialized as a single string in memory.
        const CONTENT_MATERIALIZABLE = 1 << 0;
        /// Supports snapshot/restore for undo.
        const SNAPSHOTTABLE = 1 << 1;
        /// Content is backed by a stream (mmap, network, etc.).
        const STREAMABLE = 1 << 2;
        /// Backed by a file on disk.
        const FILE_BACKED = 1 << 3;
        /// Supports line-based read access.
        const LINE_READABLE = 1 << 4;
        /// Supports content mutation (insert, delete).
        const EDITABLE = 1 << 5;
    }
}

impl BufferCapabilities {
    /// Standard capabilities for a Rope-backed text buffer.
    pub const ROPE: Self = Self::CONTENT_MATERIALIZABLE
        .union(Self::SNAPSHOTTABLE)
        .union(Self::LINE_READABLE)
        .union(Self::EDITABLE);

    /// Standard capabilities for a `VirtualBuffer` (mmap-backed large file).
    pub const VIRTUAL: Self = Self::SNAPSHOTTABLE
        .union(Self::STREAMABLE)
        .union(Self::FILE_BACKED)
        .union(Self::LINE_READABLE)
        .union(Self::EDITABLE);
}

#[cfg(test)]
#[path = "tests/buffer_caps.rs"]
mod tests;
