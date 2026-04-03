//! Testing utilities for `reovim-provider-text`.
//!
//! Provides `setup_buffer` and related helpers for test modules that need
//! to create and register `Buffer` instances in a `KernelContext`.
//!
//! This module is unconditionally compiled (not `#[cfg(test)]`) so that
//! downstream crates can use it in their test code.
//!
//! # Usage
//!
//! ```ignore
//! use reovim_kernel::testing::create_test_context;
//! use reovim_provider_text::testing::setup_buffer;
//!
//! let ctx = create_test_context();
//! let buffer_id = setup_buffer(&ctx, "hello world");
//! ```

use std::sync::Arc;

use {
    super::Buffer,
    reovim_kernel::api::v1::{BufferId, BufferOps, KernelContext, RwLock},
};

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
