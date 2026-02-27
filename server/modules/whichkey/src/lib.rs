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

pub use bridge::WhichKeyBridge;

use {
    reovim_driver_session::bridges::BridgeProvider,
    reovim_kernel::api::v1::{Module, ModuleContext, ModuleError, ModuleId, ProbeResult, Version},
};

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
}

#[cfg(feature = "dynamic")]
reovim_module_macros::declare_module!(WhichKeyModule);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_module_id() {
        let module = WhichKeyModule::new();
        assert_eq!(module.id().as_str(), "whichkey");
    }

    #[test]
    fn test_module_name() {
        let module = WhichKeyModule::new();
        assert_eq!(module.name(), "whichkey");
    }

    #[test]
    fn test_module_version() {
        let module = WhichKeyModule::new();
        let version = module.version();
        assert_eq!(version.major, 0);
        assert_eq!(version.minor, 1);
    }

    #[test]
    #[allow(clippy::default_constructed_unit_structs)]
    fn test_module_default() {
        let module = WhichKeyModule::default();
        assert_eq!(module.id().as_str(), "whichkey");
    }

    #[test]
    fn test_module_exit() {
        let mut module = WhichKeyModule::new();
        assert!(module.exit().is_ok());
    }

    #[test]
    fn test_module_init_registers_bridge() {
        use {reovim_kernel::api::v1::ServiceRegistry, std::sync::Arc};

        let services = Arc::new(ServiceRegistry::new());
        let ctx = test_module_context(services.clone());

        let mut module = WhichKeyModule::new();
        let result = module.init(&ctx);
        assert!(matches!(result, ProbeResult::Success));

        // Verify bridge was registered
        let provider = services.get::<BridgeProvider>().unwrap();
        let bridges = provider.take_bridges();
        assert_eq!(bridges.len(), 1);
        assert_eq!(bridges[0].kind(), "whichkey");
    }

    /// Create a minimal `ModuleContext` for testing.
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_module_context(
        services: std::sync::Arc<reovim_kernel::api::v1::ServiceRegistry>,
    ) -> ModuleContext {
        ModuleContext::new(
            reovim_kernel::api::v1::KernelContext::default(),
            services,
            std::path::PathBuf::from("/tmp"),
            std::path::PathBuf::from("/tmp"),
        )
    }
}
