//! Window Operations Module
//!
//! This module handles window lifecycle and viewport events from the kernel EventBus.
//! It subscribes to WindowCreated, WindowClosed, WindowFocused, and ViewportScrolled
//! events and provides coordinated window state management.
//!
//! # Architecture
//!
//! Following the kernel's "mechanism vs policy" principle:
//! - Kernel provides the window events (mechanism)
//! - This module decides how to react (policy)
//!
//! # Event Subscriptions
//!
//! - `WindowCreated`: Initialize window-specific state
//! - `WindowClosed`: Cleanup window-specific resources
//! - `WindowFocused`: Update active window tracking, trigger highlights
//! - `ViewportScrolled`: Handle lazy loading, update visible ranges

use std::sync::Arc;

use reovim_kernel::api::v1::{
    EventResult, Module, ModuleContext, ModuleError, ModuleId, ProbeResult, Subscription, Version,
    events::kernel::{ViewportScrolled, WindowClosed, WindowCreated, WindowFocused, priority},
    pr_info,
};

/// Window operations module.
///
/// Manages window lifecycle events and viewport tracking across the editor.
/// Subscriptions are stored to keep handlers active until module exit.
pub struct WindowOps {
    /// Active subscriptions - MUST be stored to prevent immediate drop (RAII pattern)
    subscriptions: Vec<Subscription>,
}

impl WindowOps {
    /// Create a new WindowOps module instance.
    pub const fn new() -> Self {
        Self {
            subscriptions: Vec::new(),
        }
    }
}

impl Default for WindowOps {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for WindowOps {
    fn id(&self) -> ModuleId {
        ModuleId::new("window-ops")
    }

    fn name(&self) -> &'static str {
        "Window Operations"
    }

    fn version(&self) -> Version {
        Version::new(0, 1, 0)
    }

    fn init(&mut self, ctx: &ModuleContext) -> ProbeResult {
        let bus = Arc::clone(&ctx.kernel.event_bus);

        // Subscribe to window creation events
        // Priority: CORE (10) - system state change, needs early handling
        let sub_created =
            bus.subscribe_with_context::<WindowCreated, _>(priority::CORE, |event, _ctx| {
                pr_info!("Window {} created", event.window_id);
                // Future: Initialize window-specific module state (status line, borders)
                // _ctx.request_render() available when needed
                EventResult::Handled
            });
        self.subscriptions.push(sub_created);

        // Subscribe to window close events
        // Priority: LOW (200) - cleanup, run after other handlers
        let sub_closed =
            bus.subscribe_with_context::<WindowClosed, _>(priority::LOW, |event, _ctx| {
                pr_info!("Window {} closed", event.window_id);
                // Future: Cleanup window-specific module state
                EventResult::Handled
            });
        self.subscriptions.push(sub_closed);

        // Subscribe to window focus events
        // Priority: CORE (10) - affects active window state
        let sub_focused =
            bus.subscribe_with_context::<WindowFocused, _>(priority::CORE, |event, _ctx| {
                pr_info!("Window focus: {:?} -> {}", event.from, event.to);
                // Future: Update status line, trigger cursor highlight
                // _ctx.request_render() available when needed
                EventResult::Handled
            });
        self.subscriptions.push(sub_focused);

        // Subscribe to viewport scroll events
        // Priority: NORMAL (50) - standard priority for viewport updates
        let sub_scrolled =
            bus.subscribe_with_context::<ViewportScrolled, _>(priority::NORMAL, |event, _ctx| {
                pr_info!(
                    "Window {} viewport scrolled: buffer={}, lines {}..{}",
                    event.window_id,
                    event.buffer_id,
                    event.top_line,
                    event.bottom_line
                );
                // Future: Trigger lazy syntax highlighting for visible range
                // _ctx.request_render() available when needed
                EventResult::Handled
            });
        self.subscriptions.push(sub_scrolled);

        pr_info!("WindowOps module initialized with {} subscriptions", self.subscriptions.len());
        ProbeResult::Success
    }

    fn exit(&mut self) -> Result<(), ModuleError> {
        // Subscriptions auto-unsubscribe when dropped (RAII)
        let count = self.subscriptions.len();
        self.subscriptions.clear();
        pr_info!("WindowOps module exiting, cleared {} subscriptions", count);
        Ok(())
    }
}

// Note: declare_module! is NOT used here because this module is statically linked.
// The macro generates FFI entry points with fixed symbol names that conflict when
// multiple modules are linked into the same binary. It's only needed for dynamic
// loading via libloading.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_module_id() {
        let module = WindowOps::new();
        assert_eq!(module.id().as_str(), "window-ops");
    }

    #[test]
    fn test_module_version() {
        let module = WindowOps::new();
        let version = module.version();
        assert_eq!(version.major, 0);
        assert_eq!(version.minor, 1);
        assert_eq!(version.patch, 0);
    }
}
