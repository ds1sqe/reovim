//! Kernel context for drivers and modules.
//!
//! Provides a unified context struct bundling kernel services for easy access.

use std::{fmt, sync::Arc};

use crate::{
    core::{MotionEngine, TextObjectEngine},
    ipc::EventBus,
};

use super::buffer_manager::BufferManager;

// ============================================================================
// KernelContext
// ============================================================================

/// Kernel context bundling all kernel services.
///
/// This struct provides drivers and modules with access to kernel services.
/// All fields are `Arc`-wrapped for cheap cloning and safe concurrent access.
///
/// # Example
///
/// ```ignore
/// use reovim_kernel::api::v1::KernelContext;
///
/// fn setup_driver(ctx: KernelContext) {
///     // Access event bus
///     let _bus = ctx.event_bus.clone();
///
///     // Access buffer manager
///     let count = ctx.buffers.count();
///
///     // Context is cheap to clone
///     let ctx2 = ctx.clone();
/// }
/// ```
#[derive(Clone)]
pub struct KernelContext {
    /// Event bus for publish/subscribe communication.
    pub event_bus: Arc<EventBus>,
    /// Buffer manager for buffer storage and retrieval.
    pub buffers: Arc<dyn BufferManager>,
    /// Motion calculation engine.
    pub motion: Arc<MotionEngine>,
    /// Text object calculation engine.
    pub text_objects: Arc<TextObjectEngine>,
}

impl KernelContext {
    /// Create a new kernel context.
    pub fn new(
        event_bus: Arc<EventBus>,
        buffers: Arc<dyn BufferManager>,
        motion: Arc<MotionEngine>,
        text_objects: Arc<TextObjectEngine>,
    ) -> Self {
        Self {
            event_bus,
            buffers,
            motion,
            text_objects,
        }
    }
}

impl fmt::Debug for KernelContext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("KernelContext")
            .field("event_bus", &"Arc<EventBus>")
            .field("buffers", &"Arc<dyn BufferManager>")
            .field("motion", &"Arc<MotionEngine>")
            .field("text_objects", &"Arc<TextObjectEngine>")
            .finish()
    }
}
