//! Session state containing kernel and buffer data.

use std::sync::Arc;

use {
    parking_lot::RwLock,
    reovim_kernel::api::v1::{Buffer, BufferId, KernelContext},
};

/// Session state containing the kernel context and buffers.
///
/// This is the core data structure that holds the editing state.
/// Access is protected by `RwLock` for thread-safe mutations.
pub struct SessionState {
    /// The kernel context with buffer manager.
    pub kernel: KernelContext,
}

impl SessionState {
    /// Create a new session state with default kernel context.
    #[must_use]
    pub fn new() -> Self {
        Self {
            kernel: KernelContext::default(),
        }
    }

    /// Get a buffer by ID.
    #[must_use]
    pub fn buffer(&self, id: BufferId) -> Option<Arc<RwLock<Buffer>>> {
        self.kernel.buffers.get(id)
    }

    /// Get the active buffer ID (if any).
    #[must_use]
    pub fn active_buffer(&self) -> Option<BufferId> {
        // For now, return the first buffer if any exist
        self.kernel.buffers.list().first().copied()
    }

    /// Create a new buffer with the given content.
    pub fn create_buffer(&mut self, content: &str) -> BufferId {
        let mut buffer = Buffer::new();
        buffer.set_content(content);
        self.kernel.buffers.register(buffer)
    }
}

impl Default for SessionState {
    fn default() -> Self {
        Self::new()
    }
}

impl SessionState {
    /// Create a session state with a custom kernel context.
    ///
    /// Useful for testing with non-default buffer managers.
    #[must_use]
    pub const fn with_kernel(kernel: KernelContext) -> Self {
        Self { kernel }
    }
}
