#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! Window Operations Module
//!
//! This module handles window lifecycle and viewport events from the kernel `EventBus`.
//! It subscribes to `WindowCreated`, `WindowClosed`, `WindowFocused`, and
//! `reovim_domain_text_events::ViewportScrolled`
//! events and provides coordinated window state management.
//!
//! # Architecture
//!
//! Following the kernel's "mechanism vs policy" principle:
//! - Kernel provides the window events (mechanism)
//! - This module decides how to react (policy)
//!
//! # Command IDs
//!
//! This module also provides command ID constants for window operations
//! (focus, split, resize, etc.) used by keybinding modules like `vim`.
//!
//! # Commands
//!
//! This module implements command handlers for window operations:
//! - Focus navigation: `<C-w>h/j/k/l` - move focus directionally
//! - Focus cycling: `<C-w>w/W` - cycle through windows
//! - Splitting: `<C-w>s/v` - horizontal/vertical splits
//! - Closing: `<C-w>c/o` - close current/close others
//! - Float zone: Toggle, raise, lower floating windows
//!
//! # Event Subscriptions
//!
//! - `WindowCreated`: Initialize window-specific state
//! - `WindowClosed`: Cleanup window-specific resources
//! - `WindowFocused`: Update active window tracking, trigger highlights
//! - `ViewportScrolled` (text-domain): Handle lazy loading, update visible ranges

pub mod command;
pub mod ids;

use std::sync::Arc;

use {
    reovim_driver_command::{CommandHandler, CommandHandlerStore, CommandProvider},
    reovim_kernel::api::v1::{
        EventResult, Module, ModuleContext, ModuleError, ModuleId, ProbeResult, Subscription,
        Version,
        events::kernel::{WindowClosed, WindowCreated, WindowFocused, priority},
        pr_info,
    },
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
    /// Create a new `WindowOps` module instance.
    #[must_use]
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

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn init(&mut self, ctx: &ModuleContext) -> ProbeResult {
        // Register command handlers (Epic #417 Part 3)
        let command_store = ctx.services.get_or_create::<CommandHandlerStore>();
        for handler in self.command_handlers() {
            command_store.add(handler);
        }

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
        // Detach so handlers survive module drop (bootstrap drops modules after init).
        sub_created.detach();
        self.subscriptions.push(sub_created);

        // Subscribe to window close events
        // Priority: LOW (200) - cleanup, run after other handlers
        let sub_closed =
            bus.subscribe_with_context::<WindowClosed, _>(priority::LOW, |event, _ctx| {
                pr_info!("Window {} closed", event.window_id);
                // Future: Cleanup window-specific module state
                EventResult::Handled
            });
        sub_closed.detach();
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
        sub_focused.detach();
        self.subscriptions.push(sub_focused);

        // Subscribe to text-domain viewport scroll events (#740 Plan 09 Phase 7c)
        // Priority: NORMAL (50) - standard priority for viewport updates
        let sub_scrolled = bus
            .subscribe_with_context::<reovim_domain_text_events::ViewportScrolled, _>(
                priority::NORMAL,
                |event, _ctx| {
                    pr_info!(
                        "Window {} viewport scrolled: buffer={}, lines {}..{}",
                        event.window_id.as_usize(),
                        event.buffer_id.as_usize(),
                        event.top_line,
                        event.bottom_line
                    );
                    // Future: Trigger lazy syntax highlighting for visible range
                    // _ctx.request_render() available when needed
                    EventResult::Handled
                },
            );
        sub_scrolled.detach();
        self.subscriptions.push(sub_scrolled);

        pr_info!(
            "WindowOps module initialized with {} commands and {} subscriptions",
            command::all_commands().len(),
            self.subscriptions.len()
        );
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

impl CommandProvider for WindowOps {
    fn command_handlers(&self) -> Vec<Box<dyn CommandHandler>> {
        command::all_commands()
    }
}

// Generate FFI entry points for dynamic loading (only when building standalone cdylib)
#[cfg(feature = "dynamic")]
reovim_module_macros::declare_module!(WindowOps);

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
