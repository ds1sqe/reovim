#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! Emacs personality skeleton for reovim (#623).
//!
//! Demonstrates that "personality" = module providing `MODE_MANAGEMENT` capability.
//! The vim module is the default personality; this module provides a minimal emacs
//! skeleton as proof-of-concept for cross-personality support.
//!
//! # Architecture
//!
//! A personality module:
//! 1. Provides `MODE_MANAGEMENT` capability
//! 2. Registers keybindings for its key-chord vocabulary
//! 3. Registers mode definitions (emacs has one primary mode + minibuffer)
//!
//! # Current Status
//!
//! This is a skeleton — it registers basic emacs-like C-x and C-c chord
//! bindings as a proof-of-concept. Full emacs emulation is future work.

mod keybindings;

use reovim_kernel::api::v1::{
    KeybindingRegistration, Module, ModuleContext, ModuleError, ModuleId, ProbeResult, Version,
};

/// Emacs personality module.
///
/// Provides emacs-like keybindings and mode management as an alternative
/// to the default vim personality.
pub struct EmacsModule;

impl EmacsModule {
    /// Create a new emacs personality module.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Default for EmacsModule {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for EmacsModule {
    fn id(&self) -> ModuleId {
        ModuleId::new("emacs")
    }

    fn name(&self) -> &'static str {
        "Emacs Personality"
    }

    fn version(&self) -> Version {
        Version::new(0, 1, 0)
    }

    fn provides(&self) -> &[&'static str] {
        &[reovim_capabilities::MODE_MANAGEMENT]
    }

    fn keybindings(&self) -> Vec<KeybindingRegistration> {
        keybindings::all()
    }

    fn init(&mut self, _ctx: &ModuleContext) -> ProbeResult {
        // Skeleton: no commands registered yet.
        // Future: register emacs ex-commands (M-x, C-x C-f, etc.)
        ProbeResult::Success
    }

    fn exit(&mut self) -> Result<(), ModuleError> {
        Ok(())
    }
}

#[cfg(feature = "dynamic")]
reovim_module_macros::declare_module!(EmacsModule);

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
