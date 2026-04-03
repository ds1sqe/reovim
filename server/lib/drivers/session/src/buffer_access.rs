//! Buffer read-access service for bridges (#664).
//!
//! Wraps `Arc<dyn BufferManager>` as a `Service` so bridges can
//! read buffer content via `ServiceRegistry` during `tick()`.

use std::sync::Arc;

use reovim_kernel::api::v1::{BufferId, BufferManager, BufferOps, RwLock, Service};

/// Read-only buffer access for bridge tick functions.
///
/// Registered at session creation so bridges can read buffer content
/// without direct access to `KernelContext`.
pub struct BufferReadAccess(Arc<dyn BufferManager>);

impl BufferReadAccess {
    /// Create a new buffer access wrapper.
    #[must_use]
    pub fn new(buffers: Arc<dyn BufferManager>) -> Self {
        Self(buffers)
    }

    #[must_use]
    pub fn get(&self, id: BufferId) -> Option<Arc<RwLock<dyn BufferOps>>> {
        self.0.get(id)
    }
}

impl Service for BufferReadAccess {}

#[cfg(test)]
#[path = "buffer_access_tests.rs"]
mod tests;
