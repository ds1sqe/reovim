//! Keymap utilities module.
//!
//! This module provides **mechanism** utilities for keybinding management:
//! - [`InteractorRegistry`] - Input routing policy for components
//! - [`InteractorConfig`] - Configuration for input handling behavior
//! - [`ComponentId`] - Identifiers for interactor components
//!
//! # Architecture
//!
//! This is a **MECHANISM** module - it provides utilities for keybinding
//! management without defining any specific bindings.
//!
//! For actual keybindings, see the `vim` module (or future `emacs`, `kakoune`
//! policy modules).
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────┐
//! │  POLICY MODULES (vim/, emacs/, etc.)       BINDINGS     │
//! │  → "hjkl moves cursor" (Vim)                            │
//! │  → "C-n moves cursor down" (Emacs)                      │
//! ├─────────────────────────────────────────────────────────┤
//! │  MECHANISM MODULES (this module)           UTILITIES    │
//! │  → InteractorRegistry (input routing)                   │
//! │  → Future: bind(), bind_mode() helpers                  │
//! └─────────────────────────────────────────────────────────┘
//! ```
//!
//! # Example
//!
//! ```
//! use reovim_module_keymap::{InteractorRegistry, InteractorConfig, ComponentId};
//!
//! let mut registry = InteractorRegistry::new();
//!
//! // Register Window mode as using keymap (h/j/k/l navigation)
//! registry.register(ComponentId::WINDOW, InteractorConfig::using_keymap());
//!
//! // Check if a component accepts character input
//! assert!(!registry.accepts_char_input(&ComponentId::WINDOW));
//! ```

use reovim_kernel::api::v1::{
    KeybindingRegistration, Module, ModuleContext, ModuleError, ModuleId, ProbeResult, Version,
    pr_info,
};

mod interactor;

// Re-export interactor types (mechanism for input routing)
pub use interactor::{ComponentId, InteractorConfig, InteractorRegistry};

/// Keymap utilities module.
///
/// Provides mechanism utilities for keybinding management.
/// Does NOT provide any keybindings - those come from policy modules (vim, emacs, etc.).
pub struct KeymapModule;

impl KeymapModule {
    /// Create a new keymap utilities module.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Default for KeymapModule {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for KeymapModule {
    fn id(&self) -> ModuleId {
        ModuleId::new("keymap")
    }

    fn name(&self) -> &'static str {
        "Keymap Utilities"
    }

    fn version(&self) -> Version {
        Version::new(0, 9, 0)
    }

    fn init(&mut self, _ctx: &ModuleContext) -> ProbeResult {
        pr_info!("Keymap utilities module initialized");
        ProbeResult::Success
    }

    fn exit(&mut self) -> Result<(), ModuleError> {
        pr_info!("Keymap utilities module exiting");
        Ok(())
    }

    fn keybindings(&self) -> Vec<KeybindingRegistration> {
        // No keybindings - this is a mechanism module.
        // Keybindings come from policy modules (vim, emacs, etc.).
        vec![]
    }
}

// Generate FFI entry points for dynamic loading (only when building standalone cdylib)
#[cfg(feature = "dynamic")]
reovim_module_macros::declare_module!(KeymapModule);

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // KeymapModule construction
    // ========================================================================

    #[test]
    fn test_keymap_module_new() {
        let _module = KeymapModule::new();
    }

    #[test]
    fn test_keymap_module_default() {
        let module = <KeymapModule as Default>::default();
        assert_eq!(module.id().as_str(), "keymap");
    }

    #[test]
    fn test_keymap_module_new_and_default_are_equivalent() {
        let m1 = KeymapModule::new();
        let m2 = <KeymapModule as Default>::default();
        assert_eq!(m1.id(), m2.id());
        assert_eq!(m1.name(), m2.name());
        assert_eq!(m1.version(), m2.version());
    }

    // ========================================================================
    // Module trait: identity methods
    // ========================================================================

    #[test]
    fn test_keymap_module_id() {
        let module = KeymapModule::new();
        assert_eq!(module.id().as_str(), "keymap");
    }

    #[test]
    fn test_keymap_module_name() {
        let module = KeymapModule::new();
        assert_eq!(module.name(), "Keymap Utilities");
    }

    #[test]
    fn test_keymap_module_version() {
        let module = KeymapModule::new();
        let version = module.version();
        assert_eq!(version.major, 0);
        assert_eq!(version.minor, 9);
        assert_eq!(version.patch, 0);
        assert_eq!(version.to_string(), "0.9.0");
    }

    #[test]
    fn test_keymap_module_api_version() {
        let module = KeymapModule::new();
        // Should return the default API version from the kernel
        let api = module.api_version();
        assert_eq!(api, reovim_kernel::api::v1::API_VERSION);
    }

    // ========================================================================
    // Module trait: lifecycle
    // ========================================================================

    #[test]
    fn test_keymap_module_init_succeeds() {
        let mut module = KeymapModule::new();
        let ctx = ModuleContext::default();
        let result = module.init(&ctx);
        assert_eq!(result, ProbeResult::Success);
    }

    #[test]
    fn test_keymap_module_exit_succeeds() {
        let mut module = KeymapModule::new();
        let result = module.exit();
        assert!(result.is_ok());
    }

    #[test]
    fn test_keymap_module_init_then_exit() {
        let mut module = KeymapModule::new();
        let ctx = ModuleContext::default();
        assert_eq!(module.init(&ctx), ProbeResult::Success);
        assert!(module.exit().is_ok());
    }

    // ========================================================================
    // Module trait: registrations
    // ========================================================================

    #[test]
    fn test_keymap_module_has_no_keybindings() {
        let module = KeymapModule::new();
        let bindings = module.keybindings();
        assert!(bindings.is_empty(), "Keymap module is mechanism-only, should have no bindings");
    }

    #[test]
    fn test_keymap_module_has_no_commands() {
        let module = KeymapModule::new();
        let cmds = module.commands();
        assert!(cmds.is_empty());
    }

    #[test]
    fn test_keymap_module_has_no_event_handlers() {
        let module = KeymapModule::new();
        let handlers = module.event_handlers();
        assert!(handlers.is_empty());
    }

    // ========================================================================
    // Module trait: dependencies
    // ========================================================================

    #[test]
    fn test_keymap_module_has_no_dependencies() {
        let module = KeymapModule::new();
        assert!(module.dependencies().is_empty());
    }

    #[test]
    fn test_keymap_module_has_no_optional_dependencies() {
        let module = KeymapModule::new();
        assert!(module.optional_dependencies().is_empty());
    }

    // ========================================================================
    // Module trait: hot reload
    // ========================================================================

    #[test]
    fn test_keymap_module_does_not_support_hot_reload() {
        let module = KeymapModule::new();
        assert!(!module.supports_hot_reload());
    }

    #[test]
    fn test_keymap_module_save_state_returns_none() {
        let module = KeymapModule::new();
        assert!(module.save_state().is_none());
    }

    #[test]
    fn test_keymap_module_restore_state_returns_error() {
        let mut module = KeymapModule::new();
        let result = module.restore_state(&[1, 2, 3]);
        assert!(result.is_err());
    }

    // ========================================================================
    // Module trait: lifecycle hooks (default no-ops)
    // ========================================================================

    #[test]
    fn test_keymap_module_on_all_loaded_is_noop() {
        let mut module = KeymapModule::new();
        let ctx = ModuleContext::default();
        // Should not panic
        module.on_all_loaded(&ctx);
    }

    #[test]
    fn test_keymap_module_on_buffer_focus_is_noop() {
        let mut module = KeymapModule::new();
        let ctx = ModuleContext::default();
        let buffer_id = reovim_kernel::api::v1::BufferId::from_raw(0);
        // Should not panic
        module.on_buffer_focus(buffer_id, &ctx);
    }

    #[test]
    fn test_keymap_module_on_unload_succeeds() {
        let mut module = KeymapModule::new();
        assert!(module.on_unload().is_ok());
    }

    // ========================================================================
    // Thread safety
    // ========================================================================

    #[test]
    fn test_keymap_module_is_send() {
        fn assert_send<T: Send>() {}
        assert_send::<KeymapModule>();
    }

    #[test]
    fn test_keymap_module_is_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<KeymapModule>();
    }

    // ========================================================================
    // Re-exports verification
    // ========================================================================

    #[test]
    fn test_interactor_registry_exists() {
        // Verify mechanism utilities are exported
        let _registry = InteractorRegistry::new();
    }

    #[test]
    fn test_interactor_config_exported() {
        let _config = InteractorConfig::accepting_input();
    }

}
