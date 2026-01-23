//! Undo module for reovim.
//!
//! Provides per-buffer undo/redo with integrated persistence to disk.
//!
//! # Architecture
//!
//! Following the mechanism/policy separation:
//! - **Mechanism**: `UndoProvider` trait (in `reovim-driver-undo`)
//! - **Policy**: `UndoRegistry` (this module) provides implementation with persistence
//!
//! # Epic #417 Part 2
//!
//! Persistence is now internal to `UndoRegistry`. The `UndoProvider` trait
//! includes `persist()` and `load()` methods that implementations handle.
//! Runner no longer needs `UndoPersistence` - it queries `dyn UndoProvider`
//! from `ServiceRegistry`.

mod registry;

pub use registry::{UndoRegistry, decode_path_component, encode_path_component};

// Re-export error type from driver for backwards compat
pub use reovim_driver_undo::UndoPersistError;

use std::sync::Arc;

use {
    reovim_driver_undo::{UndoKey, UndoProviderRegistry},
    reovim_kernel::api::v1::{
        Module, ModuleContext, ModuleError, ModuleId, ProbeResult, Version, pr_info,
    },
};

/// Undo module instance.
///
/// Provides per-buffer undo/redo functionality.
pub struct UndoModule;

impl UndoModule {
    /// Create a new undo module.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Default for UndoModule {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for UndoModule {
    fn id(&self) -> ModuleId {
        ModuleId::new("undo")
    }

    fn name(&self) -> &'static str {
        "Undo"
    }

    fn version(&self) -> Version {
        Version::new(0, 9, 0)
    }

    fn init(&mut self, ctx: &ModuleContext) -> ProbeResult {
        // Register undo provider with typed key (Epic #417)
        let undo_registry = ctx.services.get_or_create::<UndoProviderRegistry>();
        undo_registry.register(UndoKey::Buffer, Arc::new(UndoRegistry::new()));

        pr_info!("Undo module initialized");
        ProbeResult::Success
    }

    fn exit(&mut self) -> Result<(), ModuleError> {
        pr_info!("Undo module exiting");
        Ok(())
    }
}

// Generate FFI entry points for dynamic loading (only when building standalone cdylib)
#[cfg(feature = "dynamic")]
reovim_module_macros::declare_module!(UndoModule);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_module_id() {
        let module = UndoModule::new();
        assert_eq!(module.id().as_str(), "undo");
    }

    #[test]
    fn test_module_name() {
        let module = UndoModule::new();
        assert_eq!(module.name(), "Undo");
    }

    #[test]
    fn test_module_version() {
        let module = UndoModule::new();
        let version = module.version();
        assert_eq!(version.major, 0);
        assert_eq!(version.minor, 9);
        assert_eq!(version.patch, 0);
    }
}
