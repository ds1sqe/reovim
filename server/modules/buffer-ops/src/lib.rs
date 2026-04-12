#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! Buffer Operations Module
//!
//! This module handles buffer lifecycle events from the kernel `EventBus`.
//! It subscribes to `BufferCreated`, `TextBufferModified`, `BufferClosed`, and `BufferSwitched`
//! events and provides coordinated buffer state management.
//!
//! # Architecture
//!
//! Following the kernel's "mechanism vs policy" principle:
//! - Kernel provides the buffer events (mechanism)
//! - This module decides how to react (policy)
//!
//! # Event Subscriptions
//!
//! - `BufferCreated`: Initialize buffer-specific state
//! - `TextBufferModified` (text-domain): Track dirty state, schedule reparse
//! - `BufferClosed`: Cleanup buffer-specific resources
//! - `BufferSwitched`: Update active buffer tracking

use std::sync::Arc;

use reovim_kernel::api::v1::{
    EventResult, Module, ModuleContext, ModuleError, ModuleId, ProbeResult, Subscription, Version,
    events::kernel::{BufferClosed, BufferCreated, BufferSwitched, priority},
    pr_info,
};

/// Buffer operations module.
///
/// Manages buffer lifecycle events and coordinates buffer-related state
/// across the editor. Subscriptions are stored to keep handlers active
/// until module exit.
pub struct BufferOps {
    /// Active subscriptions - MUST be stored to prevent immediate drop (RAII pattern)
    subscriptions: Vec<Subscription>,
}

impl BufferOps {
    /// Create a new `BufferOps` module instance.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            subscriptions: Vec::new(),
        }
    }
}

impl Default for BufferOps {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for BufferOps {
    fn id(&self) -> ModuleId {
        ModuleId::new("buffer-ops")
    }

    fn name(&self) -> &'static str {
        "Buffer Operations"
    }

    fn version(&self) -> Version {
        Version::new(0, 1, 0)
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn init(&mut self, ctx: &ModuleContext) -> ProbeResult {
        let bus = Arc::clone(&ctx.kernel.event_bus);

        // Subscribe to buffer creation events
        // Priority: CORE (10) - system state change, needs early handling
        let sub_created =
            bus.subscribe_with_context::<BufferCreated, _>(priority::CORE, |event, _ctx| {
                pr_info!("Buffer {} created", event.buffer_id);
                // Future: Initialize buffer-specific module state
                // _ctx.request_render() available when needed
                EventResult::Handled
            });
        // Detach so handlers survive module drop (bootstrap drops modules after init).
        sub_created.detach();
        self.subscriptions.push(sub_created);

        // Subscribe to text-domain buffer modification events (#740 Plan 09 Phase 7c)
        // Priority: NORMAL (50) - content change, standard priority
        let sub_modified = bus
            .subscribe_with_context::<reovim_domain_text_events::TextBufferModified, _>(
                priority::NORMAL,
                |event, _ctx| {
                    pr_info!("Buffer {} modified: {:?}", event.buffer_id.as_usize(), event.edit);
                    // Future: Trigger treesitter reparse, update dirty flags
                    EventResult::Handled
                },
            );
        sub_modified.detach();
        self.subscriptions.push(sub_modified);

        // Subscribe to buffer close events
        // Priority: LOW (200) - cleanup, run after other handlers
        let sub_closed =
            bus.subscribe_with_context::<BufferClosed, _>(priority::LOW, |event, _ctx| {
                pr_info!("Buffer {} closed", event.buffer_id);
                // Future: Cleanup buffer-specific module state
                EventResult::Handled
            });
        sub_closed.detach();
        self.subscriptions.push(sub_closed);

        // Subscribe to buffer switch events
        // Priority: CORE (10) - affects active buffer state
        let sub_switched =
            bus.subscribe_with_context::<BufferSwitched, _>(priority::CORE, |event, _ctx| {
                pr_info!("Buffer switched: {:?} -> {}", event.from, event.to);
                // Future: Update status line, trigger highlights
                EventResult::Handled
            });
        sub_switched.detach();
        self.subscriptions.push(sub_switched);

        pr_info!("BufferOps module initialized with {} subscriptions", self.subscriptions.len());
        ProbeResult::Success
    }

    fn exit(&mut self) -> Result<(), ModuleError> {
        // Subscriptions auto-unsubscribe when dropped (RAII)
        let count = self.subscriptions.len();
        self.subscriptions.clear();
        pr_info!("BufferOps module exiting, cleared {} subscriptions", count);
        Ok(())
    }
}

// Generate FFI entry points for dynamic loading (only when building standalone cdylib)
#[cfg(feature = "dynamic")]
reovim_module_macros::declare_module!(BufferOps);

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
