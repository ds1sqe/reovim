//! Buffer read-access service for bridges (#664).
//!
//! Provides text buffer access via `TextBufferRegistry` as a `Service`
//! so bridges can read buffer content via `ServiceRegistry` during `tick()`.

use std::sync::Arc;

use reovim_kernel::api::v1::{BufferId, RwLock, Service};
use reovim_provider_text::BufferOps;

use crate::TextBufferRegistry;

/// Read-only buffer access for bridge tick functions.
///
/// Wraps `TextBufferRegistry` to provide text buffer access (`dyn BufferOps`)
/// without direct access to `KernelContext`.
pub struct BufferReadAccess(Arc<TextBufferRegistry>);

impl BufferReadAccess {
    /// Create a new buffer access wrapper.
    #[must_use]
    pub const fn new(registry: Arc<TextBufferRegistry>) -> Self {
        Self(registry)
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
