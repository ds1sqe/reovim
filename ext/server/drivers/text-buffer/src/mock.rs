//! Mock buffer manager for testing.
//!
//! Provides a working `BufferManager` implementation for use in tests.
//! This is in the driver layer (mechanism) rather than the module layer (policy)
//! because test infrastructure is mechanism, not policy.
//!
//! # Usage
//!
//! ```ignore
//! use reovim_driver_text_buffer::TestBufferManager;
//! use reovim_provider_text::BufferOps;
//! use std::sync::Arc;
//!
//! let mgr = TestBufferManager::new();
//! let buf = Buffer::new();
//! let arc: Arc<RwLock<dyn KernelBuffer>> = Arc::new(RwLock::new(buf));
//! let id = mgr.register(arc);
//! assert!(mgr.get(id).is_some());
//! ```

use std::{collections::HashMap, sync::Arc};

use {
    reovim_arch::sync::RwLock,
    reovim_kernel::api::v1::{BufferId, BufferManager, KernelBuffer},
};

/// Test buffer manager for testing purposes.
///
/// A fully functional `BufferManager` implementation that stores buffers
/// in memory. Unlike the kernel's private `StubBufferManager` (which does
/// nothing), this implementation actually works.
///
/// # Design Note
///
/// This lives in the driver layer (mechanism) so that:
/// - Tests in server can use it without depending on modules
/// - It follows mechanism/policy separation
/// - Module implementations remain in server/modules/
pub struct TestBufferManager {
    /// Buffer storage with outer `RwLock` protecting the `HashMap`.
    buffers: RwLock<HashMap<BufferId, Arc<RwLock<dyn KernelBuffer>>>>,
}

impl TestBufferManager {
    /// Create a new empty test buffer manager.
    #[must_use]
    pub fn new() -> Self {
        Self {
            buffers: RwLock::new(HashMap::new()),
        }
    }
}

impl Default for TestBufferManager {
    fn default() -> Self {
        Self::new()
    }
}

impl BufferManager for TestBufferManager {
    fn get(&self, id: BufferId) -> Option<Arc<RwLock<dyn KernelBuffer>>> {
        self.buffers.read().get(&id).cloned()
    }

    fn register(&self, buffer: Arc<RwLock<dyn KernelBuffer>>) -> BufferId {
        let id = buffer.read().id();
        self.buffers.write().insert(id, buffer);
        id
    }

    fn unregister(&self, id: BufferId) -> Option<Arc<RwLock<dyn KernelBuffer>>> {
        self.buffers.write().remove(&id)
    }

    fn list(&self) -> Vec<BufferId> {
        self.buffers.read().keys().copied().collect()
    }

    fn count(&self) -> usize {
        self.buffers.read().len()
    }
}

#[cfg(test)]
#[path = "mock_tests.rs"]
mod tests;
