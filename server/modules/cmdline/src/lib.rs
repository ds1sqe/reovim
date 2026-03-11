#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! Command-line mode module - POLICY layer.
//!
//! This module owns all command-line mode state and behavior:
//! - [`CmdlineState`] - per-client session extension
//! - [`CmdlinePrompt`] - prompt type enum (`:`, `/`, `?`)
//! - [`CmdlineBridge`] - adapts state to JSON for gRPC
//!
//! # Architecture (#468)
//!
//! `CmdlineState` was moved here from the driver layer because it is POLICY
//! (defines HOW command-line mode behaves). The driver layer only provides
//! the trait contracts (MECHANISM).
//!
//! The bridge is registered via `BridgeProvider` during `init()`, enabling
//! generic bridge detection without hardcoded server/driver code.

mod bridge;
mod state;

pub use {
    bridge::CmdlineBridge,
    state::{CmdlineMessage, CmdlinePrompt, CmdlineState},
};

use {
    reovim_driver_session::bridges::BridgeProvider,
    reovim_kernel::api::v1::{Module, ModuleContext, ModuleError, ModuleId, ProbeResult, Version},
};

const MODULE_ID: ModuleId = ModuleId::new("cmdline");

/// Command-line mode module.
///
/// Registers `CmdlineBridge` via `BridgeProvider` during `init()`.
pub struct CmdlineModule;

impl CmdlineModule {
    /// Create a new instance.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Default for CmdlineModule {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for CmdlineModule {
    fn id(&self) -> ModuleId {
        MODULE_ID
    }

    fn name(&self) -> &'static str {
        "cmdline"
    }

    fn version(&self) -> Version {
        Version::new(0, 1, 0)
    }

    fn init(&mut self, ctx: &ModuleContext) -> ProbeResult {
        // Register CmdlineBridge via BridgeProvider (#468)
        let provider = ctx.services.get_or_create::<BridgeProvider>();
        provider.register(CmdlineBridge);
        ProbeResult::Success
    }

    fn exit(&mut self) -> Result<(), ModuleError> {
        Ok(())
    }
}

#[cfg(feature = "dynamic")]
reovim_module_macros::declare_module!(CmdlineModule);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_module_id() {
        let module = CmdlineModule::new();
        assert_eq!(module.id().as_str(), "cmdline");
    }

    #[test]
    fn test_module_name() {
        let module = CmdlineModule::new();
        assert_eq!(module.name(), "cmdline");
    }

    #[test]
    fn test_module_version() {
        let module = CmdlineModule::new();
        let version = module.version();
        assert_eq!(version.major, 0);
        assert_eq!(version.minor, 1);
    }

    #[test]
    #[allow(clippy::default_constructed_unit_structs)]
    fn test_module_default() {
        let module = CmdlineModule::default();
        assert_eq!(module.id().as_str(), "cmdline");
    }

    #[test]
    fn test_module_exit() {
        let mut module = CmdlineModule::new();
        assert!(module.exit().is_ok());
    }

    #[test]
    fn test_module_init_registers_bridge() {
        use {reovim_kernel::api::v1::ServiceRegistry, std::sync::Arc};

        let services = Arc::new(ServiceRegistry::new());
        let ctx = test_module_context(services.clone());

        let mut module = CmdlineModule::new();
        let result = module.init(&ctx);
        assert!(matches!(result, ProbeResult::Success));

        // Verify bridge was registered
        let provider = services.get::<BridgeProvider>().unwrap();
        let bridges = provider.take_bridges();
        assert_eq!(bridges.len(), 1);
        assert_eq!(bridges[0].kind(), "cmdline");
    }

    /// Create a minimal `ModuleContext` for testing.
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_module_context(
        services: std::sync::Arc<reovim_kernel::api::v1::ServiceRegistry>,
    ) -> ModuleContext {
        use {
            parking_lot::RwLock,
            reovim_kernel::api::v1::{
                EventBus, KernelContext, MarkBank, MotionEngine, OptionRegistry, TextObjectEngine,
            },
            std::sync::Arc,
        };

        let kernel = KernelContext::new(
            Arc::new(EventBus::new()),
            Arc::new(reovim_driver_buffer::TestBufferManager::new()),
            Arc::new(MotionEngine),
            Arc::new(TextObjectEngine),
            Arc::new(RwLock::new(MarkBank::new())),
            Arc::new(OptionRegistry::new()),
            services.clone(),
        );
        ModuleContext::new(
            kernel,
            services,
            std::path::PathBuf::from("/tmp"),
            std::path::PathBuf::from("/tmp"),
        )
    }
}
