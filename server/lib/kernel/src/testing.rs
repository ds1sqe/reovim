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
    Buffer, BufferId, BufferManager, BufferOps, EventBus, KernelContext, MarkBank, ModeId,
    ModuleId, MotionEngine, OptionRegistry, RwLock, ServiceRegistry, TextObjectEngine,
};

/// In-memory buffer manager for testing.
///
/// Unlike `KernelContext::default()` which uses a `StubBufferManager`
/// (returns `None` for all lookups), this implementation actually stores
/// and retrieves buffers. Use this when tests need real buffer operations.
pub struct TestBufferManager {
    buffers: RwLock<HashMap<BufferId, Arc<RwLock<dyn BufferOps>>>>,
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
    fn get(&self, id: BufferId) -> Option<Arc<RwLock<dyn BufferOps>>> {
        self.buffers.read().get(&id).cloned()
    }

    fn register(&self, buffer: Arc<RwLock<dyn BufferOps>>) -> BufferId {
        let id = buffer.read().id();
        self.buffers.write().insert(id, buffer);
        id
    }

    fn unregister(&self, id: BufferId) -> Option<Arc<RwLock<dyn BufferOps>>> {
        self.buffers.write().remove(&id)
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
/// `ctx.buffers.register()`. Returns the buffer's ID.
#[must_use]
pub fn setup_buffer(ctx: &KernelContext, content: &str) -> BufferId {
    let buffer = Buffer::from_string(content);
    let arc: Arc<RwLock<dyn BufferOps>> = Arc::new(RwLock::new(buffer));
    ctx.buffers.register(arc)
}
