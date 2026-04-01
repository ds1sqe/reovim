//! Virtual buffer registry for large file management.
//!
//! Provides [`VirtualBufferRegistry`] trait for storing and retrieving
//! [`VirtualBuffer`] instances.  Registered as a service in
//! [`ServiceRegistry`](super::service::ServiceRegistry), parallel to the
//! existing [`BufferManager`](super::buffer_manager::BufferManager) for
//! small files.

use std::{collections::HashMap, sync::Arc};

use reovim_arch::sync::RwLock;

use crate::mm::{BufferId, VirtualBuffer};

use super::service::Service;

// ============================================================================
// VirtualBufferRegistry Trait
// ============================================================================

/// Registry for `VirtualBuffer` instances (large file buffers).
///
/// Mirrors the [`BufferManager`](super::buffer_manager::BufferManager) API
/// but for virtual buffers backed by mmap + piece table.
///
/// # Service Registration
///
/// Register an implementation via `ServiceRegistry::register`:
/// ```ignore
/// let registry = SimpleVirtualBufferRegistry::new();
/// services.register::<dyn VirtualBufferRegistry>(Arc::new(registry));
/// ```
pub trait VirtualBufferRegistry: Send + Sync {
    /// Get a virtual buffer by ID.
    fn get(&self, id: BufferId) -> Option<Arc<RwLock<VirtualBuffer>>>;

    /// Register a virtual buffer, returning its ID.
    fn register(&self, vbuf: VirtualBuffer) -> BufferId;

    /// Unregister a virtual buffer, returning ownership.
    fn unregister(&self, id: BufferId) -> Option<VirtualBuffer>;

    /// Check if a buffer ID belongs to a virtual buffer.
    fn is_virtual(&self, id: BufferId) -> bool;

    /// List all virtual buffer IDs.
    fn list(&self) -> Vec<BufferId>;

    /// Number of registered virtual buffers.
    fn count(&self) -> usize;
}

// ============================================================================
// SimpleVirtualBufferRegistry
// ============================================================================

/// Simple `HashMap`-based implementation of [`VirtualBufferRegistry`].
///
/// Follows the same pattern as `SimpleBufferManager` in the kernel.
pub struct SimpleVirtualBufferRegistry {
    buffers: RwLock<HashMap<BufferId, Arc<RwLock<VirtualBuffer>>>>,
}

impl SimpleVirtualBufferRegistry {
    /// Create a new empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self {
            buffers: RwLock::new(HashMap::new()),
        }
    }
}

impl Default for SimpleVirtualBufferRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl VirtualBufferRegistry for SimpleVirtualBufferRegistry {
    fn get(&self, id: BufferId) -> Option<Arc<RwLock<VirtualBuffer>>> {
        self.buffers.read().get(&id).cloned()
    }

    fn register(&self, vbuf: VirtualBuffer) -> BufferId {
        let id = vbuf.id();
        self.buffers.write().insert(id, Arc::new(RwLock::new(vbuf)));
        id
    }

    fn unregister(&self, id: BufferId) -> Option<VirtualBuffer> {
        let arc = self.buffers.write().remove(&id)?;
        // Try to unwrap the Arc. If there are other references (e.g.,
        // snapshots), fall back to cloning.
        match Arc::try_unwrap(arc) {
            Ok(lock) => Some(lock.into_inner()),
            Err(arc) => Some(arc.read().clone()),
        }
    }

    fn is_virtual(&self, id: BufferId) -> bool {
        self.buffers.read().contains_key(&id)
    }

    fn list(&self) -> Vec<BufferId> {
        self.buffers.read().keys().copied().collect()
    }

    fn count(&self) -> usize {
        self.buffers.read().len()
    }
}

impl Service for SimpleVirtualBufferRegistry {}

impl std::fmt::Debug for SimpleVirtualBufferRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SimpleVirtualBufferRegistry")
            .field("count", &self.count())
            .finish()
    }
}
