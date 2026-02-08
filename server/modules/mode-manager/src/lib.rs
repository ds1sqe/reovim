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
}

// Generate FFI entry points for dynamic loading (only when building standalone cdylib)
#[cfg(feature = "dynamic")]
reovim_module_macros::declare_module!(ModeManager);

#[cfg(test)]
mod tests {
    use super::*;

    // ========== Construction and Identity ==========

    #[test]
    fn test_module_id() {
        let module = ModeManager::new();
        assert_eq!(module.id().as_str(), "mode-manager");
    }

    #[test]
    fn test_module_version() {
        let module = ModeManager::new();
        let version = module.version();
        assert_eq!(version.major, 0);
        assert_eq!(version.minor, 1);
        assert_eq!(version.patch, 0);
    }

    #[test]
    fn test_module_name() {
        let module = ModeManager::new();
        assert_eq!(module.name(), "Mode State Manager");
    }

    #[test]
    fn test_default_impl() {
        let module = ModeManager::default();
        assert_eq!(module.id().as_str(), "mode-manager");
        assert_eq!(module.name(), "Mode State Manager");
    }

    #[test]
    fn test_new_has_empty_subscriptions() {
        let module = ModeManager::new();
        assert!(module.subscriptions.is_empty());
    }

    // ========== Module trait defaults ==========

    #[test]
    fn test_api_version() {
        let module = ModeManager::new();
        let api_v = module.api_version();
        // Should return the current API_VERSION from the kernel
        assert_eq!(api_v.major, 0);
        assert_eq!(api_v.minor, 2);
        assert_eq!(api_v.patch, 0);
    }

    #[test]
    fn test_dependencies_empty() {
        let module = ModeManager::new();
        assert!(module.dependencies().is_empty());
    }

    #[test]
    fn test_optional_dependencies_empty() {
        let module = ModeManager::new();
        assert!(module.optional_dependencies().is_empty());
    }

    #[test]
    fn test_commands_empty() {
        let module = ModeManager::new();
        assert!(module.commands().is_empty());
    }

    #[test]
    fn test_keybindings_empty() {
        let module = ModeManager::new();
        assert!(module.keybindings().is_empty());
    }

    #[test]
    fn test_event_handlers_empty() {
        let module = ModeManager::new();
        assert!(module.event_handlers().is_empty());
    }

    #[test]
    fn test_supports_hot_reload_false() {
        let module = ModeManager::new();
        assert!(!module.supports_hot_reload());
    }

    #[test]
    fn test_save_state_none() {
        let module = ModeManager::new();
        assert!(module.save_state().is_none());
    }

    #[test]
    fn test_restore_state_returns_error() {
        let mut module = ModeManager::new();
        let result = module.restore_state(&[1, 2, 3]);
        assert!(result.is_err());
    }

    // ========== Lifecycle: init ==========

    #[test]
    fn test_init_success() {
        let mut module = ModeManager::new();
        let ctx = ModuleContext::default();

        let result = module.init(&ctx);
        assert_eq!(result, ProbeResult::Success);
    }

    #[test]
    fn test_init_creates_subscription() {
        let mut module = ModeManager::new();
        let ctx = ModuleContext::default();

        module.init(&ctx);
        assert_eq!(module.subscriptions.len(), 1);
    }

    #[test]
    fn test_init_called_twice_accumulates_subscriptions() {
        let mut module = ModeManager::new();
        let ctx = ModuleContext::default();

        module.init(&ctx);
        assert_eq!(module.subscriptions.len(), 1);

        module.init(&ctx);
        assert_eq!(module.subscriptions.len(), 2);
    }

    // ========== Lifecycle: exit ==========

    #[test]
    fn test_exit_success() {
        let mut module = ModeManager::new();
        let result = module.exit();
        assert!(result.is_ok());
    }

    #[test]
    fn test_exit_clears_subscriptions() {
        let mut module = ModeManager::new();
        let ctx = ModuleContext::default();

        module.init(&ctx);
        assert_eq!(module.subscriptions.len(), 1);

        let result = module.exit();
        assert!(result.is_ok());
        assert!(module.subscriptions.is_empty());
    }

    #[test]
    fn test_exit_without_init() {
        let mut module = ModeManager::new();
        let result = module.exit();
        assert!(result.is_ok());
        assert!(module.subscriptions.is_empty());
    }

    #[test]
    fn test_exit_multiple_times() {
        let mut module = ModeManager::new();
        let ctx = ModuleContext::default();

        module.init(&ctx);
        assert!(module.exit().is_ok());
        assert!(module.subscriptions.is_empty());

        // Second exit should also be fine
        assert!(module.exit().is_ok());
        assert!(module.subscriptions.is_empty());
    }

    // ========== Event handling ==========

    #[test]
    fn test_mode_changed_event_handled() {
        use reovim_kernel::api::v1::events::kernel::ModeChanged;

        let mut module = ModeManager::new();
        let ctx = ModuleContext::default();
        module.init(&ctx);

        // Emit a mode changed event on the same event bus
        let event = ModeChanged::new("Normal", "Insert");
        ctx.kernel.event_bus.emit(event);

        // If we get here without panic, the event was processed
    }

    #[test]
    fn test_mode_changed_multiple_events() {
        use reovim_kernel::api::v1::events::kernel::ModeChanged;

        let mut module = ModeManager::new();
        let ctx = ModuleContext::default();
        module.init(&ctx);

        // Emit several mode changes
        ctx.kernel
            .event_bus
            .emit(ModeChanged::new("Normal", "Insert"));
        ctx.kernel
            .event_bus
            .emit(ModeChanged::new("Insert", "Normal"));
        ctx.kernel
            .event_bus
            .emit(ModeChanged::new("Normal", "Visual"));
        ctx.kernel
            .event_bus
            .emit(ModeChanged::new("Visual", "Normal"));
    }

    #[test]
    fn test_subscription_inactive_after_exit() {
        use reovim_kernel::api::v1::events::kernel::ModeChanged;

        let mut module = ModeManager::new();
        let ctx = ModuleContext::default();
        module.init(&ctx);

        // Exit clears subscriptions
        module.exit().unwrap();

        // Emitting events should not cause issues (no handlers)
        ctx.kernel
            .event_bus
            .emit(ModeChanged::new("Normal", "Insert"));
    }

    // ========== Thread safety ==========

    #[test]
    fn test_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<ModeManager>();
    }

    // ========== Lifecycle hooks (default no-ops) ==========

    #[test]
    fn test_on_all_loaded_noop() {
        let mut module = ModeManager::new();
        let ctx = ModuleContext::default();
        // Should not panic
        module.on_all_loaded(&ctx);
    }

    #[test]
    fn test_on_buffer_focus_noop() {
        use reovim_kernel::api::v1::BufferId;
        let mut module = ModeManager::new();
        let ctx = ModuleContext::default();
        let buf_id = BufferId::from_raw(1);
        // Should not panic
        module.on_buffer_focus(buf_id, &ctx);
    }

    #[test]
    fn test_on_unload_noop() {
        let mut module = ModeManager::new();
        let result = module.on_unload();
        assert!(result.is_ok());
    }

    // ========== Full lifecycle flow ==========

    #[test]
    fn test_full_lifecycle() {
        use reovim_kernel::api::v1::events::kernel::ModeChanged;

        let mut module = ModeManager::new();
        let ctx = ModuleContext::default();

        // Init
        assert_eq!(module.init(&ctx), ProbeResult::Success);
        assert_eq!(module.subscriptions.len(), 1);

        // on_all_loaded
        module.on_all_loaded(&ctx);

        // Process events
        ctx.kernel
            .event_bus
            .emit(ModeChanged::new("Normal", "Insert"));

        // Exit
        assert!(module.exit().is_ok());
        assert!(module.subscriptions.is_empty());

        // on_unload
        assert!(module.on_unload().is_ok());
    }

    // ========== Module identity consistency ==========

    #[test]
    fn test_id_is_consistent_across_calls() {
        let module = ModeManager::new();
        let id1 = module.id();
        let id2 = module.id();
        assert_eq!(id1, id2);
    }

    #[test]
    fn test_version_display() {
        let module = ModeManager::new();
        let version = module.version();
        assert_eq!(version.to_string(), "0.1.0");
    }

    #[test]
    fn test_module_id_is_static() {
        let module = ModeManager::new();
        let id = module.id();
        assert!(id.is_static());
    }
}
