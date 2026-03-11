#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! Clipboard module for reovim.
//!
//! Provides system clipboard access via `arboard`.
//!
//! # Architecture (#515 Phase 4)
//!
//! Following the mechanism/policy separation:
//! - **Mechanism**: `ClipboardProvider` trait (in `reovim-driver-clipboard`)
//! - **Policy**: `ClipboardService` (this module) provides OS clipboard implementation
//!
//! History (numbered registers 0-9) has been moved to per-client `HistoryRing`
//! in `EditingState`, managed by `SessionRuntime::push_to_clipboard_history`.
//!
//! # Register Mapping
//!
//! | Register | Source | Description |
//! |----------|--------|-------------|
//! | `""` | Per-client `RegisterBank` | Unnamed (default) |
//! | `a-z` | Per-client `RegisterBank` | Named registers |
//! | `+` | This module (OS clipboard) | System clipboard |
//! | `*` | This module (OS selection) | Selection (X11 primary) |
//! | `0-9` | Per-client `HistoryRing` | Yank history |

mod service;

pub use service::ClipboardService;

use std::sync::Arc;

use {
    reovim_driver_clipboard::{ClipboardKey, ClipboardProviderRegistry},
    reovim_kernel::api::v1::{
        Module, ModuleContext, ModuleError, ModuleId, ProbeResult, Version, pr_info,
    },
};

/// Clipboard module instance.
///
/// Provides system clipboard access and yank history.
pub struct ClipboardModule;

impl ClipboardModule {
    /// Create a new clipboard module.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Default for ClipboardModule {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for ClipboardModule {
    fn id(&self) -> ModuleId {
        ModuleId::new("clipboard")
    }

    fn name(&self) -> &'static str {
        "Clipboard"
    }

    fn version(&self) -> Version {
        Version::new(0, 9, 0)
    }

    fn init(&mut self, ctx: &ModuleContext) -> ProbeResult {
        // Register clipboard provider with typed key
        let clipboard_registry = ctx.services.get_or_create::<ClipboardProviderRegistry>();
        clipboard_registry.register(ClipboardKey::Default, Arc::new(ClipboardService::new()));

        pr_info!("Clipboard module initialized");
        ProbeResult::Success
    }

    fn exit(&mut self) -> Result<(), ModuleError> {
        pr_info!("Clipboard module exiting");
        Ok(())
    }
}

// Generate FFI entry points for dynamic loading (only when building standalone cdylib)
#[cfg(feature = "dynamic")]
reovim_module_macros::declare_module!(ClipboardModule);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_module_id() {
        let module = ClipboardModule::new();
        assert_eq!(module.id().as_str(), "clipboard");
    }

    #[test]
    fn test_module_name() {
        let module = ClipboardModule::new();
        assert_eq!(module.name(), "Clipboard");
    }

    #[test]
    fn test_module_version() {
        let module = ClipboardModule::new();
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
        let from_new = ClipboardModule::new();
        let from_default: ClipboardModule = create_default();
        assert_eq!(from_new.id(), from_default.id());
        assert_eq!(from_new.version(), from_default.version());
    }

    #[test]
    fn test_exit_succeeds() {
        let mut module = ClipboardModule::new();
        assert!(module.exit().is_ok());
    }

    #[test]
    fn test_dependencies_default_empty() {
        let module = ClipboardModule::new();
        assert!(module.dependencies().is_empty());
    }

    #[test]
    fn test_init_registers_clipboard_provider() {
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

        let mut module = ClipboardModule::new();
        let result = module.init(&ctx);
        assert_eq!(result, ProbeResult::Success);

        // Verify that ClipboardProviderRegistry was created in services
        let registry = services.get::<ClipboardProviderRegistry>();
        assert!(registry.is_some(), "ClipboardProviderRegistry should be registered");
    }
}
