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
    reovim_driver_text_buffer::{BufferManagerKey, BufferManagerRegistry},
    reovim_kernel::api::v1::{
        BufferId, BufferManager, KernelBuffer, Module, ModuleContext, ModuleError, ModuleId,
        ProbeResult, Version, pr_info,
    },
    std::collections::HashMap,
};

/// Simple buffer manager implementation.
///
/// Uses a `HashMap` with outer `RwLock` for concurrent access.
/// Each buffer is wrapped in `Arc<RwLock<dyn KernelBuffer>>` for shared ownership.
/// The kernel sees only bytes and metadata; text access is handled by
/// `TextBufferRegistry` at the session layer.
///
/// # Design Philosophy
///
/// - **No I/O operations**: File loading/saving is VFS driver's job
/// - **No syntax attachment**: Syntax is handled by modules (policy)
/// - **Thread-safe**: All operations are safe for concurrent access
/// - **Simple**: Just manages buffer storage, nothing more
pub struct SimpleBufferManager {
    /// Buffer storage with outer `RwLock` protecting the `HashMap`.
    buffers: RwLock<HashMap<BufferId, Arc<RwLock<dyn KernelBuffer>>>>,
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

    fn provides(&self) -> &[&'static str] {
        &[reovim_domain_text_capabilities::BUFFER_MANAGER]
    }
}

// Generate FFI entry points for dynamic loading (only when building standalone cdylib)
#[cfg(feature = "dynamic")]
reovim_module_macros::declare_module!(BufferSimpleModule);

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
