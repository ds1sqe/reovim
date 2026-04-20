#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! Mode State Management Module
//!
//! This module handles mode change events from the kernel `EventBus`.
//! It subscribes to `ModeChanged` events and coordinates mode-related
//! state across the editor.
//!
//! # Architecture
//!
//! Following the kernel's "mechanism vs policy" principle:
//! - Kernel provides the mode change events (mechanism)
//! - This module decides how to react (policy)
//!
//! # Event Subscriptions
//!
//! - `ModeChanged`: Track mode transitions, update UI state
//!
//! # Mode Semantics
//!
//! The kernel uses string-based modes (from/to) intentionally to keep
//! mode policy in modules. The mode strings are opaque to the kernel.
//! Examples: "Normal", "Insert", "Visual", "Command", etc.

use std::sync::Arc;

use reovim_kernel::api::v1::{
    EventResult, Module, ModuleContext, ModuleError, ModuleId, ProbeResult, Subscription, Version,
    events::kernel::{ModeChanged, priority},
    pr_info,
};

/// Mode state management module.
///
/// Tracks mode transitions and coordinates mode-dependent behavior
/// across the editor. Subscriptions are stored to keep handlers active
/// until module exit.
pub struct ModeManager {
    /// Active subscriptions - MUST be stored to prevent immediate drop (RAII pattern)
    subscriptions: Vec<Subscription>,
}

impl ModeManager {
    /// Create a new `ModeManager` module instance.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            subscriptions: Vec::new(),
        }
    }
}

impl Default for ModeManager {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for ModeManager {
    fn id(&self) -> ModuleId {
        ModuleId::new("mode-manager")
    }

    fn name(&self) -> &'static str {
        "Mode State Manager"
    }

    fn version(&self) -> Version {
        Version::new(0, 1, 0)
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn init(&mut self, ctx: &ModuleContext) -> ProbeResult {
        let bus = Arc::clone(&ctx.kernel.event_bus);

        // Subscribe to mode change events
        // Priority: CORE (10) - mode changes affect all editor behavior
        let sub_mode =
            bus.subscribe_with_context::<ModeChanged, _>(priority::CORE, |event, _ctx| {
                pr_info!("Mode changed: '{}' -> '{}'", event.from, event.to);
                // Future: Update cursor style, status line, available keybindings
                // The mode strings are policy-defined by the runtime
                // _ctx.request_render() available when needed
                EventResult::Handled
            });
        // Detach so handler survives module drop (bootstrap drops modules after init).
        sub_mode.detach();
        self.subscriptions.push(sub_mode);

        pr_info!("ModeManager module initialized with {} subscriptions", self.subscriptions.len());
        ProbeResult::Success
    }

    fn exit(&mut self) -> Result<(), ModuleError> {
        // Subscriptions auto-unsubscribe when dropped (RAII)
        let count = self.subscriptions.len();
        self.subscriptions.clear();
        pr_info!("ModeManager module exiting, cleared {} subscriptions", count);
        Ok(())
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn provides(&self) -> &[&'static str] {
        &[reovim_capabilities::MODE_MANAGEMENT]
    }
}

// Generate FFI entry points for dynamic loading (only when building standalone cdylib)
#[cfg(feature = "dynamic")]
reovim_module_macros::declare_module!(ModeManager);

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
