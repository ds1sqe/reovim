//! Simple buffer manager module for reovim.
//!
//! Provides the `SimpleBufferManager` implementation of the `BufferManager` trait.
//!
//! # Architecture
//!
//! Following the mechanism/policy separation:
//! - **Mechanism**: `BufferManager` trait (in kernel)
//! - **Policy**: `SimpleBufferManager` (this module) provides implementation
//!
//! # Design Philosophy
//!
//! - **No I/O operations**: File loading/saving is VFS driver's job
//! - **No syntax attachment**: Syntax is handled by modules (policy)
//! - **Thread-safe**: All operations are safe for concurrent access
//! - **Simple**: Just manages buffer storage, nothing more

use std::sync::Arc;

use {
    reovim_arch::sync::RwLock,
    reovim_driver_buffer::{BufferManagerKey, BufferManagerRegistry},
    reovim_kernel::api::v1::{
        Buffer, BufferError, BufferId, BufferManager, Module, ModuleContext, ModuleError, ModuleId,
        ProbeResult, Version, pr_info,
    },
    std::collections::HashMap,
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

/// Buffer simple module instance.
///
/// Provides the `SimpleBufferManager` for buffer storage.
pub struct BufferSimpleModule;

impl BufferSimpleModule {
    /// Create a new buffer simple module.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Default for BufferSimpleModule {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for BufferSimpleModule {
    fn id(&self) -> ModuleId {
        ModuleId::new("buffer-simple")
    }

    fn name(&self) -> &'static str {
        "Simple Buffer Manager"
    }

    fn version(&self) -> Version {
        Version::new(0, 9, 0)
    }

    fn init(&mut self, ctx: &ModuleContext) -> ProbeResult {
        // Register buffer manager with typed key (Epic #417)
        let buffer_registry = ctx.services.get_or_create::<BufferManagerRegistry>();
        buffer_registry.register(BufferManagerKey::Simple, Arc::new(SimpleBufferManager::new()));

        pr_info!("Buffer simple module initialized");
        ProbeResult::Success
    }

    fn exit(&mut self) -> Result<(), ModuleError> {
        pr_info!("Buffer simple module exiting");
        Ok(())
    }
}

// Generate FFI entry points for dynamic loading (only when building standalone cdylib)
#[cfg(feature = "dynamic")]
reovim_module_macros::declare_module!(BufferSimpleModule);

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

    #[test]
    fn test_module_id() {
        let module = BufferSimpleModule::new();
        assert_eq!(module.id().as_str(), "buffer-simple");
    }

    #[test]
    fn test_module_name() {
        let module = BufferSimpleModule::new();
        assert_eq!(module.name(), "Simple Buffer Manager");
    }
}
