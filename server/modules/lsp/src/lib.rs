#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! LSP module for reovim.
//!
//! Provides Language Server Protocol integration following the
//! mechanism/policy separation:
//! - **Mechanism**: `LspProvider` trait, `Client`, transport (in `reovim-driver-lsp`)
//! - **Policy**: `LspModule` (this module) manages server lifecycle and events

mod saturator;

pub use saturator::{LspSaturator, LspSaturatorHandle};

use {
    reovim_driver_lsp::LspProviderRegistry,
    reovim_kernel::api::v1::{
        Module, ModuleContext, ModuleError, ModuleId, ProbeResult, Version, pr_info,
    },
};

/// LSP module instance.
///
/// Registers the `LspProviderRegistry` in `ServiceRegistry` during init.
/// Language servers are started on-demand when files with supported
/// languages are opened.
pub struct LspModule;

impl LspModule {
    /// Create a new LSP module.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Default for LspModule {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for LspModule {
    fn id(&self) -> ModuleId {
        ModuleId::new("lsp")
    }

    fn name(&self) -> &'static str {
        "LSP"
    }

    fn version(&self) -> Version {
        Version::new(0, 9, 0)
    }

    fn init(&mut self, ctx: &ModuleContext) -> ProbeResult {
        // Create the LSP provider registry (empty initially).
        // Providers are registered on-demand when language servers are spawned.
        let _registry = ctx.services.get_or_create::<LspProviderRegistry>();

        pr_info!("LSP module initialized");
        ProbeResult::Success
    }

    fn exit(&mut self) -> Result<(), ModuleError> {
        pr_info!("LSP module exiting");
        Ok(())
    }
}

// Generate FFI entry points for dynamic loading (only when building standalone cdylib)
#[cfg(feature = "dynamic")]
reovim_module_macros::declare_module!(LspModule);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_module_id() {
        let module = LspModule::new();
        assert_eq!(module.id().as_str(), "lsp");
    }

    #[test]
    fn test_module_name() {
        let module = LspModule::new();
        assert_eq!(module.name(), "LSP");
    }

    #[test]
    fn test_module_version() {
        let module = LspModule::new();
        let version = module.version();
        assert_eq!(version.major, 0);
        assert_eq!(version.minor, 9);
        assert_eq!(version.patch, 0);
    }

    #[test]
    fn test_module_default() {
        fn create_default<T: Default>() -> T {
            T::default()
        }
        let from_default: LspModule = create_default();
        let from_new = LspModule::new();
        assert_eq!(from_new.id(), from_default.id());
        assert_eq!(from_new.version(), from_default.version());
    }

    #[test]
    fn test_exit_succeeds() {
        let mut module = LspModule::new();
        assert!(module.exit().is_ok());
    }

    #[test]
    fn test_dependencies_default_empty() {
        let module = LspModule::new();
        assert!(module.dependencies().is_empty());
    }

    #[test]
    fn test_init_registers_provider_registry() {
        use {
            reovim_kernel::api::v1::{KernelContext, ModuleContext, ServiceRegistry},
            std::{path::PathBuf, sync::Arc},
        };

        let kernel = KernelContext::default();
        let services = Arc::new(ServiceRegistry::new());
        let ctx = ModuleContext::new(
            kernel,
            services.clone(),
            PathBuf::from("/tmp/test-data"),
            PathBuf::from("/tmp/test-cache"),
        );

        let mut module = LspModule::new();
        let result = module.init(&ctx);
        assert_eq!(result, ProbeResult::Success);

        // Verify that LspProviderRegistry was created in services
        let registry = services.get::<LspProviderRegistry>();
        assert!(registry.is_some(), "LspProviderRegistry should be registered in services");
    }
}
