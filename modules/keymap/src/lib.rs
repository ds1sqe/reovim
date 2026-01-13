//! Vim keybindings module.
//!
//! Provides standard vim keybindings: hjkl movement, operators, modes.
//!
//! This is a **POLICY** module - it defines WHICH keys trigger WHICH actions.
//! The kernel provides the mechanisms (motion calculation, buffer operations).
//!
//! # Reference
//!
//! This module was concept-extracted from lib/core/src/bind/mod.rs.
//! It is NOT a migration - it's a fresh implementation using kernel APIs.
//!
//! # Example
//!
//! ```ignore
//! use reovim_kernel::api::v1::*;
//!
//! // The module registers keybindings via Module::keybindings()
//! let module = KeymapModule::new();
//! let bindings = module.keybindings();
//!
//! // Each binding maps a key sequence to a command
//! // 'j' -> cursor-down
//! // 'dd' -> delete-line
//! ```

use {reovim_kernel::api::v1::*, reovim_module_macros::declare_module};

mod insert;
mod interactor;
mod normal;
mod operator_pending;
mod visual;

// Re-export interactor types
pub use interactor::{ComponentId, InteractorConfig, InteractorRegistry};

/// Vim keybindings module.
///
/// Implements standard vim keybindings as policy.
/// The module is stateless - all state is managed by the kernel.
pub struct KeymapModule;

impl KeymapModule {
    /// Create a new keymap module.
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
        "Vim Keymap"
    }

    fn version(&self) -> Version {
        Version::new(0, 9, 0)
    }

    fn init(&mut self, _ctx: &ModuleContext) -> ProbeResult {
        pr_info!("Keymap module initialized");
        ProbeResult::Success
    }

    fn exit(&mut self) -> Result<(), ModuleError> {
        pr_info!("Keymap module exiting");
        Ok(())
    }

    fn keybindings(&self) -> Vec<KeybindingRegistration> {
        let mut bindings = Vec::new();
        bindings.extend(normal::bindings());
        bindings.extend(insert::bindings());
        bindings.extend(visual::bindings());
        bindings.extend(operator_pending::bindings());
        bindings
    }
}

/// Returns all keybindings provided by this module.
///
/// Convenience function to get all bindings without creating a module instance.
#[must_use]
pub fn bindings() -> Vec<KeybindingRegistration> {
    let mut all = Vec::new();
    all.extend(normal::bindings());
    all.extend(insert::bindings());
    all.extend(visual::bindings());
    all.extend(operator_pending::bindings());
    all
}

// Generate FFI entry points for dynamic loading
declare_module!(KeymapModule);
