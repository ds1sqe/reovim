//! Text buffer registry for session-layer text access (#740).
//!
//! Stores `Arc<RwLock<dyn BufferOps>>` independently from the kernel's
//! `BufferManager`. This decouples text-specific buffer access from the
//! kernel, allowing `BufferManager` to eventually store only byte-level
//! `dyn KernelBuffer` objects.
//!
//! # Migration Plan
//!
//! 1. `TextBufferRegistry` created alongside kernel `BufferManager` (current step)
//! 2. `SessionRuntime` text access migrates here from `kernel.buffers`
//! 3. `BufferManager` changes stored type to `dyn KernelBuffer`
//! 4. `BufferOps` definition moves out of kernel

use std::{collections::HashMap, sync::Arc};

use reovim_arch::sync::RwLock;
use reovim_kernel::api::v1::{BufferId, Service};
use reovim_provider_text::BufferOps;

/// Session-layer registry for text-specific buffer access.
///
/// Mirrors the kernel `BufferManager` interface but lives at the session
/// layer. During the #740 migration, callers switch from
/// `kernel.buffers.get(id)` to `text_buffers.get(id)` for text operations.
///
/// Uses interior mutability (`RwLock`) so it can be stored as `Arc<Self>`
/// in the `ServiceRegistry` and shared across threads.
pub struct TextBufferRegistry {
    buffers: RwLock<HashMap<BufferId, Arc<RwLock<dyn BufferOps>>>>,
}

impl TextBufferRegistry {
    /// Create a new empty text buffer registry.
    #[must_use]
    pub fn new() -> Self {
        Self {
            buffers: RwLock::new(HashMap::new()),
        }
    }

    /// Get a text buffer by ID.
    #[must_use]
    pub fn get(&self, id: BufferId) -> Option<Arc<RwLock<dyn BufferOps>>> {
        self.buffers.read().get(&id).cloned()
    }

    /// Register a text buffer.
    ///
    /// Uses the buffer's own ID (from `BufferMeta::id()`) as the key.
    /// Returns the buffer's ID.
    pub fn register(&self, buffer: Arc<RwLock<dyn BufferOps>>) -> BufferId {
        let id = buffer.read().id();
        self.buffers.write().insert(id, buffer);
        id
    }

    /// Unregister a text buffer, returning the arc if it existed.
    pub fn unregister(&self, id: BufferId) -> Option<Arc<RwLock<dyn BufferOps>>> {
        self.buffers.write().remove(&id)
    }

    /// Number of registered text buffers.
    #[must_use]
    pub fn count(&self) -> usize {
        self.buffers.read().len()
    }

    /// List all registered buffer IDs.
    #[must_use]
    pub fn list(&self) -> Vec<BufferId> {
        self.buffers.read().keys().copied().collect()
    }
}

impl Default for TextBufferRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl Service for TextBufferRegistry {}

#[cfg(test)]
#[path = "text_buffer_registry_tests.rs"]
mod tests;
