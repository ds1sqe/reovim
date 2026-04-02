//! Unified buffer handle for server-layer access.
//!
//! `BufferHandle` wraps `Arc<RwLock<dyn BufferOps>>` providing common read
//! operations for gRPC handlers. This is a thin wrapper that will be removed
//! in P3.8 once all callers migrate to `dyn BufferOps` directly.
//!
//! # Architecture
//!
//! ```text
//! Kernel:  BufferOps (trait), Buffer, VirtualBuffer  (concrete types)
//! Driver:  BufferCapabilities                        (capability flags)
//! Server:  BufferHandle (here)                       (unified access)
//! gRPC:    Uses BufferHandle for all read queries
//! ```

use std::{borrow::Cow, sync::Arc};

use {
    parking_lot::RwLock,
    reovim_driver_buffer::BufferCapabilities,
    reovim_kernel::api::v1::BufferOps,
    reovim_types_text::Position,
};

/// Unified handle for buffers at the server layer.
///
/// Wraps a single `Arc<RwLock<dyn BufferOps>>` and delegates all read
/// operations. This replaces the former two-variant enum that distinguished
/// Rope vs Virtual buffers.
pub struct BufferHandle(Arc<RwLock<dyn BufferOps>>);

impl BufferHandle {
    /// Create a new `BufferHandle` wrapping the given buffer.
    pub fn new(arc: Arc<RwLock<dyn BufferOps>>) -> Self {
        Self(arc)
    }

    /// Returns the capability flags for this buffer type.
    #[must_use]
    pub fn capabilities(&self) -> BufferCapabilities {
        self.0.read().capabilities()
    }

    /// Returns the number of lines in the buffer.
    #[must_use]
    pub fn line_count(&self) -> usize {
        self.0.read().line_count()
    }

    /// Returns a specific line by index (0-based).
    #[must_use]
    pub fn line(&self, idx: usize) -> Option<String> {
        self.0.read().line(idx).map(Cow::into_owned)
    }

    /// Returns the file path, if any.
    #[must_use]
    pub fn file_path(&self) -> Option<String> {
        self.0.read().file_path().map(String::from)
    }

    /// Returns whether the buffer has been modified since last save.
    #[must_use]
    pub fn is_modified(&self) -> bool {
        self.0.read().is_modified()
    }

    /// Returns the full content as a string.
    #[must_use]
    pub fn content(&self) -> String {
        self.0.read().content()
    }

    /// Convert a (line, column) position to a byte offset.
    #[must_use]
    pub fn position_to_byte(&self, position: Position) -> usize {
        self.0.read().position_to_byte(position)
    }
}

#[cfg(test)]
#[path = "buffer_handle_tests.rs"]
mod tests;
