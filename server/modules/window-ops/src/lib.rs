#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! Window Operations Module
//!
//! This module handles window lifecycle and viewport events from the kernel `EventBus`.
//! It subscribes to `WindowCreated`, `WindowClosed`, `WindowFocused`, and `ViewportScrolled`
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
//! - `ViewportScrolled`: Handle lazy loading, update visible ranges

pub mod command;
pub mod ids;

use std::sync::Arc;

use {
    reovim_driver_command::{CommandHandler, CommandHandlerStore, CommandProvider},
    reovim_kernel::api::v1::{
        EventResult, Module, ModuleContext, ModuleError, ModuleId, ProbeResult, Subscription,
        Version,
        events::kernel::{ViewportScrolled, WindowClosed, WindowCreated, WindowFocused, priority},
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
mod tests {
    use super::*;

    // =========================================================================
    // WindowOps construction
    // =========================================================================

    #[test]
    fn test_new_creates_empty_subscriptions() {
        let module = WindowOps::new();
        assert!(module.subscriptions.is_empty());
    }

    #[test]
    fn test_default_creates_same_as_new() {
        let from_new = WindowOps::new();
        let from_default = WindowOps::default();
        assert_eq!(from_new.subscriptions.len(), from_default.subscriptions.len());
    }

    // =========================================================================
    // Module trait implementation
    // =========================================================================

    #[test]
    fn test_module_id() {
        let module = WindowOps::new();
        assert_eq!(module.id().as_str(), "window-ops");
    }

    #[test]
    fn test_module_name() {
        let module = WindowOps::new();
        assert_eq!(module.name(), "Window Operations");
    }

    #[test]
    fn test_module_version() {
        let module = WindowOps::new();
        let version = module.version();
        assert_eq!(version.major, 0);
        assert_eq!(version.minor, 1);
        assert_eq!(version.patch, 0);
    }

    #[test]
    fn test_module_version_is_pre_release() {
        let module = WindowOps::new();
        let version = module.version();
        // Version 0.x.y is pre-release
        assert_eq!(version.major, 0);
    }

    // =========================================================================
    // Exit behavior
    // =========================================================================

    #[test]
    fn test_exit_clears_subscriptions() {
        let mut module = WindowOps::new();
        // Initially empty
        assert!(module.subscriptions.is_empty());
        // Exit should succeed even with no subscriptions
        let result = module.exit();
        assert!(result.is_ok());
        assert!(module.subscriptions.is_empty());
    }

    #[test]
    fn test_exit_returns_ok() {
        let mut module = WindowOps::new();
        assert!(module.exit().is_ok());
    }

    // =========================================================================
    // CommandProvider implementation
    // =========================================================================

    #[test]
    fn test_command_provider_returns_handlers() {
        let module = WindowOps::new();
        let handlers = module.command_handlers();
        assert_eq!(handlers.len(), 28);
    }

    #[test]
    fn test_command_provider_matches_all_commands() {
        let module = WindowOps::new();
        let handlers = module.command_handlers();
        let all = command::all_commands();
        assert_eq!(handlers.len(), all.len());
    }

    // =========================================================================
    // Init with real kernel context (event bus integration)
    // =========================================================================

    /// Helper to create a `ModuleContext` for testing.
    fn make_test_module_context() -> ModuleContext {
        use {
            reovim_kernel::api::v1::{EventBus, KernelContext, ServiceRegistry},
            std::path::PathBuf,
        };

        let event_bus = Arc::new(EventBus::new());
        let services = Arc::new(ServiceRegistry::new());
        let kernel = KernelContext::with_event_bus_services_and_options(
            event_bus,
            Arc::clone(&services),
            Arc::new(reovim_kernel::api::v1::OptionRegistry::new()),
        );

        ModuleContext::new(
            kernel,
            services,
            PathBuf::from("/tmp/reovim-test/data"),
            PathBuf::from("/tmp/reovim-test/cache"),
        )
    }

    #[test]
    fn test_init_creates_subscriptions() {
        let ctx = make_test_module_context();

        let mut module = WindowOps::new();
        let result = module.init(&ctx);

        assert_eq!(result, ProbeResult::Success);
        // Should have 4 subscriptions: WindowCreated, WindowClosed, WindowFocused, ViewportScrolled
        assert_eq!(module.subscriptions.len(), 4);
    }

    #[test]
    fn test_init_then_exit_clears_all_subscriptions() {
        let ctx = make_test_module_context();

        let mut module = WindowOps::new();
        module.init(&ctx);
        assert_eq!(module.subscriptions.len(), 4);

        module.exit().unwrap();
        assert!(module.subscriptions.is_empty());
    }

    #[test]
    fn test_init_registers_commands_in_service_registry() {
        let ctx = make_test_module_context();

        let mut module = WindowOps::new();
        module.init(&ctx);

        // The CommandHandlerStore should be accessible from services
        let store = ctx.services.get::<CommandHandlerStore>();
        assert!(store.is_some(), "CommandHandlerStore should be registered in services");
    }

    #[test]
    fn test_double_exit_is_safe() {
        let ctx = make_test_module_context();

        let mut module = WindowOps::new();
        module.init(&ctx);
        assert_eq!(module.subscriptions.len(), 4);

        // First exit
        assert!(module.exit().is_ok());
        assert!(module.subscriptions.is_empty());

        // Second exit should also be safe (no-op)
        assert!(module.exit().is_ok());
        assert!(module.subscriptions.is_empty());
    }

    #[test]
    fn test_event_handler_window_created() {
        use reovim_kernel::api::v1::events::kernel::WindowCreated;

        let ctx = make_test_module_context();
        let mut module = WindowOps::new();
        module.init(&ctx);

        // Publish a WindowCreated event to exercise the subscription closure
        ctx.kernel.event_bus.emit(WindowCreated { window_id: 1 });

        // If we get here without panicking, the handler ran successfully
        module.exit().unwrap();
    }

    #[test]
    fn test_event_handler_window_closed() {
        use reovim_kernel::api::v1::events::kernel::WindowClosed;

        let ctx = make_test_module_context();
        let mut module = WindowOps::new();
        module.init(&ctx);

        ctx.kernel.event_bus.emit(WindowClosed { window_id: 1 });

        module.exit().unwrap();
    }

    #[test]
    fn test_event_handler_window_focused() {
        use reovim_kernel::api::v1::events::kernel::WindowFocused;

        let ctx = make_test_module_context();
        let mut module = WindowOps::new();
        module.init(&ctx);

        ctx.kernel.event_bus.emit(WindowFocused {
            from: Some(1),
            to: 2,
        });

        module.exit().unwrap();
    }

    #[test]
    fn test_event_handler_viewport_scrolled() {
        use reovim_kernel::api::v1::events::kernel::ViewportScrolled;

        let ctx = make_test_module_context();
        let mut module = WindowOps::new();
        module.init(&ctx);

        ctx.kernel.event_bus.emit(ViewportScrolled {
            window_id: 1,
            buffer_id: 1,
            top_line: 0,
            bottom_line: 50,
        });

        module.exit().unwrap();
    }

    #[test]
    fn test_dependencies_default_empty() {
        let module = WindowOps::new();
        assert!(module.dependencies().is_empty());
    }
}
