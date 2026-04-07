//! Per-buffer byte-level undo log registry (#740).
//!
//! Stores a [`ByteUndoLog`] per buffer via the [`ServiceRegistry`] pattern.
//! The session runtime pushes [`ByteEdit`] records here after every mutation,
//! making byte-level undo available across codec/provider switches.
//!
//! # Architecture
//!
//! ```text
//! ServiceRegistry
//!   └─ ByteUndoRegistry
//!        └─ HashMap<BufferId, ByteUndoLog>
//! ```
//!
//! [`ByteUndoLog`]: reovim_kernel::api::v1::ByteUndoLog
//! [`ByteEdit`]: reovim_kernel::api::v1::ByteEdit
//! [`ServiceRegistry`]: reovim_kernel::api::v1::ServiceRegistry

use std::collections::HashMap;

use {
    reovim_arch::sync::RwLock,
    reovim_kernel::api::v1::{BufferId, ByteEdit, ByteUndoLog, Service},
};

/// Per-buffer byte-level undo log storage.
///
/// Uses interior mutability (`RwLock`) so it can be stored as `Arc<Self>`
/// in the `ServiceRegistry` and shared across threads.
pub struct ByteUndoRegistry {
    logs: RwLock<HashMap<BufferId, ByteUndoLog>>,
}

impl ByteUndoRegistry {
    /// Create a new empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self {
            logs: RwLock::new(HashMap::new()),
        }
    }

    /// Push a byte edit to the undo log for a buffer.
    ///
    /// Creates the log on first use (lazy initialization).
    pub fn push(&self, buffer_id: BufferId, edits: Vec<ByteEdit>) {
        self.logs.write().entry(buffer_id).or_default().push(edits);
    }

    /// Check if a buffer can undo.
    #[must_use]
    pub fn can_undo(&self, buffer_id: BufferId) -> bool {
        self.logs
            .read()
            .get(&buffer_id)
            .is_some_and(ByteUndoLog::can_undo)
    }

    /// Check if a buffer can redo.
    #[must_use]
    pub fn can_redo(&self, buffer_id: BufferId) -> bool {
        self.logs
            .read()
            .get(&buffer_id)
            .is_some_and(ByteUndoLog::can_redo)
    }

    /// Remove the undo log for a buffer.
    pub fn remove(&self, buffer_id: BufferId) {
        self.logs.write().remove(&buffer_id);
    }

    /// Number of buffers with undo logs.
    #[must_use]
    pub fn len(&self) -> usize {
        self.logs.read().len()
    }

    /// Check if no buffers have undo logs.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.logs.read().is_empty()
    }

    /// Clear all undo logs.
    pub fn clear(&self) {
        self.logs.write().clear();
    }
}

impl Default for ByteUndoRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl Service for ByteUndoRegistry {}

#[cfg(test)]
#[path = "byte_undo_registry_tests.rs"]
mod tests;
