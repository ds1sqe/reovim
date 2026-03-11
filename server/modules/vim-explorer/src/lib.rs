//! Vim keybinding adapter for file explorer commands.
//!
//! This adapter module bridges `vim` (editor personality) and
//! `explorer` (file tree sidebar). It maps the vim normal-mode
//! leader key to the explorer toggle:
//! - `<Space>e` → Toggle file explorer
//!
//! # Architecture
//!
//! This is an adapter (integration) module. It exists because
//! `explorer` should not know about vim's mode names:
//! - `explorer` provides commands, modes, and tree UI (no vim knowledge)
//! - `vim` provides modes and core keybindings (no explorer knowledge)
//! - This module bridges them with explicit, visible coupling

use {
    reovim_driver_input::KeybindingStore,
    reovim_kernel::api::v1::{
        KeybindingRegistration, Module, ModuleContext, ModuleError, ModuleId, ProbeResult, Version,
    },
    reovim_module_explorer::ids as explorer,
};

const MODULE: ModuleId = ModuleId::new("vim-explorer");

/// Vim-explorer keybinding adapter module.
///
/// Registers the explorer toggle keybinding in `vim:normal` mode,
/// targeting the toggle command defined in `explorer`.
pub struct VimExplorerModule;

impl VimExplorerModule {
    /// Create a new instance.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Default for VimExplorerModule {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for VimExplorerModule {
    fn id(&self) -> ModuleId {
        MODULE
    }

    fn name(&self) -> &'static str {
        "Vim Explorer Keybindings"
    }

    fn version(&self) -> Version {
        Version::new(0, 1, 0)
    }

    fn init(&mut self, ctx: &ModuleContext) -> ProbeResult {
        let store = ctx.services.get_or_create::<KeybindingStore>();
        store.add_all(self.keybindings());
        ProbeResult::Success
    }

    fn exit(&mut self) -> Result<(), ModuleError> {
        Ok(())
    }

    fn keybindings(&self) -> Vec<KeybindingRegistration> {
        vec![
            KeybindingRegistration::new("<Space>e", explorer::TOGGLE)
                .with_modes(&["vim:normal"])
                .with_category("explorer")
                .with_description("Toggle file explorer"),
        ]
    }
}

#[cfg(feature = "dynamic")]
reovim_module_macros::declare_module!(VimExplorerModule);

#[cfg(test)]
mod tests {
    use std::{path::PathBuf, sync::Arc};

    use super::*;

    #[test]
    fn module_id() {
        let module = VimExplorerModule::new();
        assert_eq!(module.id().as_str(), "vim-explorer");
    }

    #[test]
    fn module_name() {
        let module = VimExplorerModule::new();
        assert_eq!(module.name(), "Vim Explorer Keybindings");
    }

    #[test]
    fn module_version() {
        let module = VimExplorerModule::new();
        let v = module.version();
        assert_eq!((v.major, v.minor, v.patch), (0, 1, 0));
    }

    #[test]
    #[allow(clippy::default_constructed_unit_structs)]
    fn module_default() {
        let module = VimExplorerModule::default();
        assert_eq!(module.id().as_str(), "vim-explorer");
    }

    #[test]
    fn module_exit() {
        let mut module = VimExplorerModule::new();
        assert!(module.exit().is_ok());
    }

    #[test]
    fn init_registers_keybindings() {
        use reovim_kernel::api::v1::ServiceRegistry;

        let services = Arc::new(ServiceRegistry::new());
        let ctx = test_module_context(services.clone());

        let mut module = VimExplorerModule::new();
        let result = module.init(&ctx);
        assert!(matches!(result, ProbeResult::Success));

        let store = services.get::<KeybindingStore>();
        assert!(store.is_some());
    }

    #[test]
    fn keybindings_count() {
        let module = VimExplorerModule::new();
        assert_eq!(module.keybindings().len(), 1);
    }

    #[test]
    fn keybindings_keys() {
        let module = VimExplorerModule::new();
        assert!(module.keybindings().iter().any(|kb| kb.keys == "<Space>e"));
    }

    #[test]
    fn keybindings_target_vim_normal() {
        let module = VimExplorerModule::new();
        for kb in module.keybindings() {
            assert!(kb.modes.contains(&"vim:normal"));
        }
    }

    #[test]
    fn keybindings_have_explorer_category() {
        let module = VimExplorerModule::new();
        for kb in module.keybindings() {
            assert_eq!(kb.category, Some("explorer"));
        }
    }

    #[test]
    fn keybindings_have_descriptions() {
        let module = VimExplorerModule::new();
        for kb in module.keybindings() {
            assert!(!kb.description.is_empty());
        }
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_module_context(
        services: Arc<reovim_kernel::api::v1::ServiceRegistry>,
    ) -> ModuleContext {
        ModuleContext::new(
            reovim_kernel::api::v1::KernelContext::default(),
            services,
            PathBuf::from("/tmp"),
            PathBuf::from("/tmp"),
        )
    }
}
