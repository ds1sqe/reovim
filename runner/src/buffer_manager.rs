//! Simple buffer manager implementation.
//!
//! Provides the concrete implementation of the `BufferManager` trait
//! for the runner application. Ported from `archive/runner/src/buffer_manager.rs`.

use {
    reovim_arch::sync::RwLock,
    reovim_kernel::api::v1::{Buffer, BufferError, BufferId, BufferManager},
    std::{collections::HashMap, sync::Arc},
};

/// Simple buffer manager implementation.
///
/// Uses a `HashMap` with outer `RwLock` for concurrent access.
/// Each buffer is wrapped in `Arc<RwLock<Buffer>>` for shared ownership.
///
/// # Design Philosophy
///
/// - **No I/O operations**: File loading/saving is VFS driver's job
/// - **No syntax attachment**: Syntax is handled by modules (policy)
/// - **Thread-safe**: All operations are safe for concurrent access
/// - **Simple**: Just manages buffer storage, nothing more
pub struct SimpleBufferManager {
    /// Buffer storage with outer `RwLock` protecting the `HashMap`.
    /// Inner `Arc<RwLock<Buffer>>` allows multiple references to the same buffer.
    buffers: RwLock<HashMap<BufferId, Arc<RwLock<Buffer>>>>,
}

impl SimpleBufferManager {
    /// Create a new empty buffer manager.
    #[must_use]
    pub fn new() -> Self {
        Self {
            buffers: RwLock::new(HashMap::new()),
        }
    }
}

impl Default for SimpleBufferManager {
    fn default() -> Self {
        Self::new()
    }
}

impl BufferManager for SimpleBufferManager {
    fn get(&self, id: BufferId) -> Option<Arc<RwLock<Buffer>>> {
        self.buffers.read().get(&id).cloned()
    }

    fn create(&self) -> BufferId {
        let id = BufferId::new(); // Uses atomic counter internally
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
                Arc::try_unwrap(arc_buffer)
                    .map_or_else(|arc| Ok(arc.read().clone()), |rwlock| Ok(rwlock.into_inner()))
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
    fn test_create_buffer() {
        let mgr = SimpleBufferManager::new();
        let id = mgr.create();
        assert!(mgr.get(id).is_some());
        assert_eq!(mgr.count(), 1);
    }

    #[test]
    fn test_register_buffer() {
        let mgr = SimpleBufferManager::new();
        let buffer = Buffer::from_string("hello");
        let id = mgr.register(buffer);
        assert!(mgr.get(id).is_some());
    }

    #[test]
    fn test_unregister_buffer() {
        let mgr = SimpleBufferManager::new();
        let id = mgr.create();
        let result = mgr.unregister(id);
        assert!(result.is_ok());
        assert!(mgr.get(id).is_none());
        assert_eq!(mgr.count(), 0);
    }

    #[test]
    fn test_unregister_not_found() {
        let mgr = SimpleBufferManager::new();
        let fake_id = BufferId::new();
        let result = mgr.unregister(fake_id);
        assert!(matches!(result, Err(BufferError::NotFound(_))));
    }

    #[test]
    fn test_list_buffers() {
        let mgr = SimpleBufferManager::new();
        let id1 = mgr.create();
        let id2 = mgr.create();
        let list = mgr.list();
        assert_eq!(list.len(), 2);
        assert!(list.contains(&id1));
        assert!(list.contains(&id2));
    }
}
