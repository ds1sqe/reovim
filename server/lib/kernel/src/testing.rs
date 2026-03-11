//! Test utilities for kernel-level testing.
//!
//! Provides shared helpers used by all modules and drivers that need
//! a real in-memory `BufferManager` for testing. This eliminates the
//! need to duplicate `TestBufferManager` and `create_test_context()`
//! across 30+ test modules.
//!
//! # Usage
//!
//! ```ignore
//! use reovim_kernel::testing::{create_test_context, setup_buffer};
//!
//! let ctx = create_test_context();
//! let buffer_id = setup_buffer(&ctx, "hello world");
//! ```
//!
//! # Architecture
//!
//! This module is unconditionally compiled (not `#[cfg(test)]`) so that
//! downstream crates can use it in their test modules. This follows the
//! same pattern as `reovim_driver_session::testing`.

use std::{collections::HashMap, sync::Arc};

use crate::api::v1::{
    Buffer, BufferError, BufferId, BufferManager, EventBus, KernelContext, MarkBank, ModeId,
    ModuleId, MotionEngine, OptionRegistry, RwLock, ServiceRegistry, TextObjectEngine,
};

/// In-memory buffer manager for testing.
///
/// Unlike `KernelContext::default()` which uses a `StubBufferManager`
/// (returns `None` for all lookups), this implementation actually stores
/// and retrieves buffers. Use this when tests need real buffer operations.
pub struct TestBufferManager {
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

#[cfg_attr(coverage_nightly, coverage(off))]
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

/// Create a `KernelContext` with a real in-memory buffer manager.
///
/// This is the standard test context used across all modules. It provides:
/// - Real `TestBufferManager` (stores and retrieves buffers)
/// - Fresh `EventBus`
/// - Default `MotionEngine`, `TextObjectEngine`
/// - Empty `MarkBank`, `OptionRegistry`, `ServiceRegistry`
///
/// For tests that need services (undo, search, etc.), create a context
/// with this function and then register services on `ctx.services`.
#[must_use]
pub fn create_test_context() -> KernelContext {
    KernelContext::new(
        Arc::new(EventBus::new()),
        Arc::new(TestBufferManager::new()),
        Arc::new(MotionEngine),
        Arc::new(TextObjectEngine),
        Arc::new(RwLock::new(MarkBank::new())),
        Arc::new(OptionRegistry::new()),
        Arc::new(ServiceRegistry::new()),
    )
}

/// Standard test mode ID for unit tests.
///
/// Returns `ModeId::new(ModuleId::new("test"), "normal")`.
/// Use this instead of defining `fn test_mode()` locally in test modules.
#[must_use]
pub const fn test_mode() -> ModeId {
    ModeId::new(ModuleId::new("test"), "normal")
}

/// Create a buffer with content and register it in the context.
///
/// Convenience helper that combines `Buffer::from_string()` and
/// `ctx.buffers.register()`.
#[must_use]
pub fn setup_buffer(ctx: &KernelContext, content: &str) -> BufferId {
    let buffer = Buffer::from_string(content);
    ctx.buffers.register(buffer)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_buffer_manager_create_and_get() {
        let manager = TestBufferManager::new();
        let id = manager.create();
        assert!(manager.get(id).is_some());
    }

    #[test]
    fn test_buffer_manager_register_and_get() {
        let manager = TestBufferManager::new();
        let buffer = Buffer::from_string("hello");
        let id = manager.register(buffer);
        let retrieved = manager.get(id).unwrap();
        assert_eq!(retrieved.read().line(0), Some("hello"));
    }

    #[test]
    fn test_buffer_manager_get_nonexistent() {
        let manager = TestBufferManager::new();
        assert!(manager.get(BufferId::from_raw(999)).is_none());
    }

    #[test]
    fn test_buffer_manager_unregister() {
        let manager = TestBufferManager::new();
        let buffer = Buffer::from_string("test");
        let id = manager.register(buffer);
        let unregistered = manager.unregister(id).unwrap();
        assert_eq!(unregistered.line(0), Some("test"));
        assert!(manager.get(id).is_none());
    }

    #[test]
    fn test_buffer_manager_unregister_nonexistent() {
        let manager = TestBufferManager::new();
        let id = BufferId::from_raw(999);
        assert_eq!(manager.unregister(id).unwrap_err(), BufferError::NotFound(id));
    }

    #[test]
    fn test_buffer_manager_unregister_with_extra_ref() {
        let manager = TestBufferManager::new();
        let buffer = Buffer::from_string("shared");
        let id = manager.register(buffer);
        // Hold an extra reference to prevent Arc::try_unwrap from succeeding
        let _extra_ref = manager.get(id).unwrap();
        let unregistered = manager.unregister(id).unwrap();
        assert_eq!(unregistered.line(0), Some("shared"));
    }

    #[test]
    fn test_buffer_manager_list() {
        let manager = TestBufferManager::new();
        assert!(manager.list().is_empty());
        let id1 = manager.create();
        let id2 = manager.create();
        let list = manager.list();
        assert_eq!(list.len(), 2);
        assert!(list.contains(&id1));
        assert!(list.contains(&id2));
    }

    #[test]
    fn test_buffer_manager_count() {
        let manager = TestBufferManager::new();
        assert_eq!(manager.count(), 0);
        manager.create();
        assert_eq!(manager.count(), 1);
        manager.create();
        assert_eq!(manager.count(), 2);
    }

    #[test]
    fn test_buffer_manager_default() {
        let manager = TestBufferManager::default();
        assert_eq!(manager.count(), 0);
    }

    #[test]
    fn test_create_test_context() {
        let ctx = create_test_context();
        // Buffer manager should work (not the stub)
        let buffer = Buffer::from_string("test content");
        let id = ctx.buffers.register(buffer);
        let retrieved = ctx.buffers.get(id);
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().read().line(0), Some("test content"));
    }

    #[test]
    fn test_setup_buffer() {
        let ctx = create_test_context();
        let id = setup_buffer(&ctx, "hello\nworld");
        let buf = ctx.buffers.get(id).unwrap();
        let guard = buf.read();
        assert_eq!(guard.line(0), Some("hello"));
        assert_eq!(guard.line(1), Some("world"));
        drop(guard);
    }
}
