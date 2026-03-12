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
#[path = "lib_tests.rs"]
mod tests;
