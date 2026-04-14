#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! Which-key popup module - POLICY layer.
//!
//! Shows available keybinding completions when the user has a pending
//! key sequence (e.g., after pressing `g` in normal mode).
//!
//! # Architecture (#468)
//!
//! This module reads [`PendingBindings`] from driver-input -- a generic
//! extension populated by the session layer after any resolver returns
//! `Pending`. It has **zero dependency on vim** and works with any
//! editor module that uses the keymap system.
//!
//! The bridge is registered via [`BridgeProvider`] during `init()`.

mod bridge;
pub mod filter;

pub use {
    bridge::WhichKeyBridge,
    filter::{BindingLayerFilter, WhichKeyFilterConfig},
};

use {
    reovim_driver_text_session::bridges::BridgeProvider,
    reovim_kernel::api::v1::{Module, ModuleContext, ModuleError, ModuleId, ProbeResult, Version},
};

const KIND: &str = "whichkey";
const MODULE_ID: ModuleId = ModuleId::new("whichkey");

/// Which-key popup module.
///
/// Registers [`WhichKeyBridge`] via [`BridgeProvider`] during `init()`.
pub struct WhichKeyModule;

impl WhichKeyModule {
    /// Create a new instance.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Default for WhichKeyModule {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for WhichKeyModule {
    fn id(&self) -> ModuleId {
        MODULE_ID
    }

    fn name(&self) -> &'static str {
        "whichkey"
    }

    fn version(&self) -> Version {
        Version::new(0, 1, 0)
    }

    fn init(&mut self, ctx: &ModuleContext) -> ProbeResult {
        // Register WhichKeyBridge via BridgeProvider (#468)
        let provider = ctx.services.get_or_create::<BridgeProvider>();
        provider.register(WhichKeyBridge);
        ProbeResult::Success
    }

    fn exit(&mut self) -> Result<(), ModuleError> {
        Ok(())
    }

    fn extension_kinds(&self) -> &[&'static str] {
        &[KIND]
    }
}

#[cfg(feature = "dynamic")]
reovim_module_macros::declare_module!(WhichKeyModule);

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
