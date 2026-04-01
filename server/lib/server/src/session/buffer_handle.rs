//! Unified buffer handle for server-layer access.
//!
//! `BufferHandle` wraps both Rope-backed `Buffer` and `VirtualBuffer` behind
//! a single enum, providing common read operations for gRPC handlers.
//!
//! # Architecture
//!
//! ```text
//! Kernel:  Buffer, VirtualBuffer           (concrete types)
//! Driver:  BufferCapabilities              (capability flags)
//! Server:  BufferHandle (here)             (unified access)
//! gRPC:    Uses BufferHandle for all read queries
//! ```

use std::sync::Arc;

use {
    parking_lot::RwLock,
    reovim_driver_buffer::BufferCapabilities,
    reovim_kernel::api::v1::{Buffer, Position, VirtualBuffer},
};

/// Unified handle for both buffer types at the server layer.
///
/// gRPC handlers receive this from `SessionState::buffer()` and use
/// the common methods for read operations. For type-specific mutations,
/// callers can match on the enum variants directly.
pub enum BufferHandle {
    /// Rope-backed text buffer (small/medium files).
    Rope(Arc<RwLock<Buffer>>),
    /// Virtual buffer (large files, mmap-backed).
    Virtual(Arc<RwLock<VirtualBuffer>>),
}

impl BufferHandle {
    /// Returns the capability flags for this buffer type.
    #[must_use]
    pub const fn capabilities(&self) -> BufferCapabilities {
        match self {
            Self::Rope(_) => BufferCapabilities::ROPE,
            Self::Virtual(_) => BufferCapabilities::VIRTUAL,
        }
    }

    /// Returns the number of lines in the buffer.
    #[must_use]
    pub fn line_count(&self) -> usize {
        match self {
            Self::Rope(arc) => arc.read().line_count(),
            Self::Virtual(arc) => arc.read().line_count(),
        }
    }

    /// Returns a specific line by index (0-based).
    #[must_use]
    pub fn line(&self, idx: usize) -> Option<String> {
        match self {
            Self::Rope(arc) => arc.read().line(idx).map(String::from),
            Self::Virtual(arc) => arc.read().line(idx),
        }
    }

    /// Returns the file path, if any.
    #[must_use]
    pub fn file_path(&self) -> Option<String> {
        match self {
            Self::Rope(arc) => arc.read().file_path().map(String::from),
            Self::Virtual(arc) => arc.read().file_path().map(String::from),
        }
    }

    /// Returns whether the buffer has been modified since last save.
    #[must_use]
    pub fn is_modified(&self) -> bool {
        match self {
            Self::Rope(arc) => arc.read().is_modified(),
            Self::Virtual(arc) => arc.read().is_modified(),
        }
    }

    /// Returns the full content as a string.
    ///
    /// For Rope buffers, this returns the Rope's string content.
    /// For virtual buffers, this materializes from pieces.
    #[must_use]
    pub fn content(&self) -> String {
        match self {
            Self::Rope(arc) => arc.read().content(),
            Self::Virtual(arc) => arc.read().content(),
        }
    }

    /// Convert a (line, column) position to a byte offset.
    #[must_use]
    pub fn position_to_byte(&self, position: Position) -> usize {
        match self {
            Self::Rope(arc) => arc.read().position_to_byte(position),
            Self::Virtual(arc) => arc.read().position_to_byte(position),
        }
    }
}

#[cfg(test)]
#[path = "buffer_handle_tests.rs"]
mod tests;
