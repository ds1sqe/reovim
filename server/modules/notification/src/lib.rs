#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! Notification module - POLICY layer.
//!
//! Provides toast notification state and bridge for server-to-client
//! notification streaming. Other modules push notifications via
//! `ext_mut::<NotificationState>().push(level, title)`.
//!
//! # Architecture (#443)
//!
//! This module reads [`NotificationState`] from the client's `ExtensionMap`
//! -- a generic session extension populated by any module that wants to
//! emit user-facing notifications. It has **zero dependency on vim** and
//! works with any editor module that uses the `ExtensionApi`.
//!
//! The bridge is registered via [`BridgeProvider`] during `init()`.

mod bridge;
mod drain;
mod state;

pub use {
    bridge::NotificationBridge,
    state::{Notification, NotificationLevel, NotificationState, Progress},
};

use {
    reovim_driver_session::{NotificationDrainRegistry, bridges::BridgeProvider},
    reovim_kernel::api::v1::{Module, ModuleContext, ModuleError, ModuleId, ProbeResult, Version},
    std::sync::Arc,
};

const MODULE_ID: ModuleId = ModuleId::new("notification");

/// Notification module.
///
/// Registers [`NotificationBridge`] via [`BridgeProvider`] during `init()`.
pub struct NotificationModule;

impl NotificationModule {
    /// Create a new instance.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Default for NotificationModule {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for NotificationModule {
    fn id(&self) -> ModuleId {
        MODULE_ID
    }

    fn name(&self) -> &'static str {
        "notification"
    }

    fn version(&self) -> Version {
        Version::new(0, 1, 0)
    }

    fn init(&mut self, ctx: &ModuleContext) -> ProbeResult {
        let provider = ctx.services.get_or_create::<BridgeProvider>();
        provider.register(NotificationBridge);

        // Register NotificationDrain implementation (#542: decouple completion from this module).
        let drain_registry = ctx.services.get_or_create::<NotificationDrainRegistry>();
        drain_registry.register(Arc::new(drain::NotificationDrainImpl));

        ProbeResult::Success
    }

    fn exit(&mut self) -> Result<(), ModuleError> {
        Ok(())
    }
}

#[cfg(feature = "dynamic")]
reovim_module_macros::declare_module!(NotificationModule);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_module_id() {
        let module = NotificationModule::new();
        assert_eq!(module.id().as_str(), "notification");
    }

    #[test]
    fn test_module_name() {
        let module = NotificationModule::new();
        assert_eq!(module.name(), "notification");
    }

    #[test]
    fn test_module_version() {
        let module = NotificationModule::new();
        let version = module.version();
        assert_eq!(version.major, 0);
        assert_eq!(version.minor, 1);
        assert_eq!(version.patch, 0);
    }

    #[test]
    #[allow(clippy::default_constructed_unit_structs)]
    fn test_module_default() {
        let module = NotificationModule::default();
        assert_eq!(module.id().as_str(), "notification");
    }

    #[test]
    fn test_module_exit() {
        let mut module = NotificationModule::new();
        assert!(module.exit().is_ok());
    }

    #[test]
    fn test_module_init_registers_bridge() {
        use {reovim_kernel::api::v1::ServiceRegistry, std::sync::Arc};

        let services = Arc::new(ServiceRegistry::new());
        let ctx = test_module_context(services.clone());

        let mut module = NotificationModule::new();
        let result = module.init(&ctx);
        assert!(matches!(result, ProbeResult::Success));

        // Verify bridge was registered
        let provider = services.get::<BridgeProvider>().unwrap();
        let bridges = provider.take_bridges();
        assert_eq!(bridges.len(), 1);
        assert_eq!(bridges[0].kind(), "notification");
    }

    #[test]
    fn test_module_init_twice_reuses_provider() {
        use {reovim_kernel::api::v1::ServiceRegistry, std::sync::Arc};

        let services = Arc::new(ServiceRegistry::new());
        let ctx = test_module_context(services.clone());

        let mut module1 = NotificationModule::new();
        let mut module2 = NotificationModule::new();
        let _ = module1.init(&ctx);
        let _ = module2.init(&ctx);

        // Both registrations go through get_or_create (second uses "get" path)
        let provider = services.get::<BridgeProvider>().unwrap();
        let bridges = provider.take_bridges();
        assert_eq!(bridges.len(), 2);
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
