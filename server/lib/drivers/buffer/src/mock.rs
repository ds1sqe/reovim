//! Mock buffer manager for testing.
//!
//! Provides a working `BufferManager` implementation for use in tests.
//! This is in the driver layer (mechanism) rather than the module layer (policy)
//! because test infrastructure is mechanism, not policy.
//!
//! # Usage
//!
//! ```ignore
//! use reovim_driver_buffer::TestBufferManager;
//!
//! let mgr = TestBufferManager::new();
//! let id = mgr.create();
//! assert!(mgr.get(id).is_some());
//! ```

use std::{collections::HashMap, sync::Arc};

use {
    reovim_arch::sync::RwLock,
    reovim_kernel::api::v1::{Buffer, BufferError, BufferId, BufferManager},
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
/// - Tests in runner can use it without depending on modules
/// - It follows mechanism/policy separation
/// - Module implementations remain in modules/
pub struct TestBufferManager {
    /// Buffer storage with outer `RwLock` protecting the `HashMap`.
    buffers: RwLock<HashMap<BufferId, Arc<RwLock<Buffer>>>>,
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
    fn get(&self, id: BufferId) -> Option<Arc<RwLock<Buffer>>> {
        self.buffers.read().get(&id).cloned()
    }

    fn create(&self) -> BufferId {
        let id = BufferId::new();
        let buffer = Arc::new(RwLock::new(Buffer::new()));
        self.buffers.write().insert(id, buffer);
        id
    }

    fn register(&self, buffer: Buffer) -> BufferId {
        let id = BufferId::new();
        let buffer = Arc::new(RwLock::new(buffer));
        self.buffers.write().insert(id, buffer);
        id
    }

    fn unregister(&self, id: BufferId) -> Result<Buffer, BufferError> {
        self.buffers
            .write()
            .remove(&id)
            .map_or(Err(BufferError::NotFound(id)), |arc_buffer| {
                // Try to unwrap the Arc. If there are other references,
                // clone the buffer (safe but creates a copy).
                match Arc::try_unwrap(arc_buffer) {
                    Ok(rwlock) => Ok(rwlock.into_inner()),
                    Err(arc) => Ok(arc.read().clone()),
                }
            })
    }

    fn list(&self) -> Vec<BufferId> {
        self.buffers.read().keys().copied().collect()
    }

    fn count(&self) -> usize {
        self.buffers.read().len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_and_get() {
        let mgr = TestBufferManager::new();
        let id = mgr.create();
        assert!(mgr.get(id).is_some());
        assert_eq!(mgr.count(), 1);
    }

    #[test]
    fn test_register() {
        let mgr = TestBufferManager::new();
        let buffer = Buffer::from_string("test content");
        let id = mgr.register(buffer);
        assert!(mgr.get(id).is_some());
    }

    #[test]
    fn test_unregister() {
        let mgr = TestBufferManager::new();
        let id = mgr.create();
        assert!(mgr.unregister(id).is_ok());
        assert!(mgr.get(id).is_none());
    }

    #[test]
    fn test_list() {
        let mgr = TestBufferManager::new();
        let id1 = mgr.create();
        let id2 = mgr.create();
        let list = mgr.list();
        assert_eq!(list.len(), 2);
        assert!(list.contains(&id1));
        assert!(list.contains(&id2));
    }
}
