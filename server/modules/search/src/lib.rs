#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! Search module for reovim.
//!
//! Provides regex-based pattern matching for Vim-style / and ? commands.
//!
//! # Architecture
//!
//! Following the mechanism/policy separation:
//! - **Mechanism**: `SearchProvider` trait (in `reovim-driver-search`)
//! - **Policy**: `SearchEngine` (this module) provides regex implementation

mod engine;

pub use engine::SearchEngine;

use std::sync::Arc;

use {
    reovim_driver_search::{SearchKey, SearchProviderRegistry},
    reovim_kernel::api::v1::{
        Module, ModuleContext, ModuleError, ModuleId, ProbeResult, Version, pr_info,
    },
};

/// Search module instance.
///
/// Provides regex-based search functionality.
pub struct SearchModule;

impl SearchModule {
    /// Create a new search module.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Default for SearchModule {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for SearchModule {
    fn id(&self) -> ModuleId {
        ModuleId::new("search")
    }

    fn name(&self) -> &'static str {
        "Search"
    }

    fn version(&self) -> Version {
        Version::new(0, 9, 0)
    }

    fn init(&mut self, ctx: &ModuleContext) -> ProbeResult {
        // Register search provider with typed key (Epic #417)
        let search_registry = ctx.services.get_or_create::<SearchProviderRegistry>();
        search_registry.register(SearchKey::Regex, Arc::new(SearchEngine));

        pr_info!("Search module initialized");
        ProbeResult::Success
    }

    fn exit(&mut self) -> Result<(), ModuleError> {
        pr_info!("Search module exiting");
        Ok(())
    }
}

// Generate FFI entry points for dynamic loading (only when building standalone cdylib)
#[cfg(feature = "dynamic")]
reovim_module_macros::declare_module!(SearchModule);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_module_id() {
        let module = SearchModule::new();
        assert_eq!(module.id().as_str(), "search");
    }

    #[test]
    fn test_module_name() {
        let module = SearchModule::new();
        assert_eq!(module.name(), "Search");
    }

    #[test]
    fn test_module_version() {
        let module = SearchModule::new();
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
        let from_default: SearchModule = create_default();
        let from_new = SearchModule::new();
        assert_eq!(from_new.id(), from_default.id());
        assert_eq!(from_new.version(), from_default.version());
    }

    #[test]
    fn test_exit_succeeds() {
        let mut module = SearchModule::new();
        assert!(module.exit().is_ok());
    }

    #[test]
    fn test_dependencies_default_empty() {
        let module = SearchModule::new();
        assert!(module.dependencies().is_empty());
    }

    #[test]
    fn test_init_registers_search_provider() {
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

        let mut module = SearchModule::new();
        let result = module.init(&ctx);
        assert_eq!(result, ProbeResult::Success);

        // Verify that SearchProviderRegistry was created in services
        let registry = services.get::<SearchProviderRegistry>();
        assert!(registry.is_some(), "SearchProviderRegistry should be registered in services");
    }
}
