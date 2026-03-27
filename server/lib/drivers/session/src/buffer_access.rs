//! Buffer read-access service for bridges (#664).
//!
//! Wraps `Arc<dyn BufferManager>` as a `Service` so bridges can
//! read buffer content via `ServiceRegistry` during `tick()`.

use std::sync::Arc;

use reovim_kernel::api::v1::{BufferManager, Service};

/// Read-only buffer access for bridge tick functions.
///
/// Registered at session creation so bridges can read buffer content
/// without direct access to `KernelContext`.
///
/// # Usage
///
/// ```ignore
/// let Some(access) = services.get::<BufferReadAccess>() else { return false; };
/// let buffer = access.manager().get(buffer_id)?;
/// let content = buffer.read().line(0);
/// ```
pub struct BufferReadAccess(Arc<dyn BufferManager>);

impl BufferReadAccess {
    /// Create a new buffer access wrapper.
    #[must_use]
    pub fn new(buffers: Arc<dyn BufferManager>) -> Self {
        Self(buffers)
    }

    /// Get the underlying buffer manager.
    #[must_use]
    pub fn manager(&self) -> &dyn BufferManager {
        &*self.0
    }
}

impl Service for BufferReadAccess {}

#[cfg(test)]
#[path = "buffer_access_tests.rs"]
mod tests;
