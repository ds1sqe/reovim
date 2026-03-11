//! Vim keybinding adapter for LSP navigation commands.
//!
//! This adapter module bridges `vim` (editor personality) and
//! `lsp-navigation` (code intelligence). It maps vim normal-mode
//! keys to LSP commands:
//! - `gd` → Go to Definition
//! - `gr` → Find References
//!
//! # Architecture
//!
//! This is an adapter (integration) module. It exists because neither
//! `vim` nor `lsp-navigation` should know about each other:
//! - `lsp-navigation` provides commands and picker (no keybinding knowledge)
//! - `vim` provides modes and core keybindings (no LSP knowledge)
//! - This module bridges them with explicit, visible coupling

use {
    reovim_driver_input::KeybindingStore,
    reovim_kernel::api::v1::{
        KeybindingRegistration, Module, ModuleContext, ModuleError, ModuleId, ProbeResult, Version,
    },
    reovim_module_lsp_navigation::ids as lsp_nav,
};

const MODULE: ModuleId = ModuleId::new("vim-lsp");

/// Vim-LSP keybinding adapter module.
///
/// Registers `gd` and `gr` keybindings in `vim:normal` mode,
/// targeting LSP navigation commands defined in `lsp-navigation`.
pub struct VimLspModule;

impl VimLspModule {
    /// Create a new instance.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Default for VimLspModule {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for VimLspModule {
    fn id(&self) -> ModuleId {
        MODULE
    }

    fn name(&self) -> &'static str {
        "Vim LSP Keybindings"
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
            KeybindingRegistration::new("gd", lsp_nav::GOTO_DEFINITION)
                .with_modes(&["vim:normal"])
                .with_category("lsp")
                .with_description("Go to definition (LSP)"),
            KeybindingRegistration::new("gr", lsp_nav::REFERENCES)
                .with_modes(&["vim:normal"])
                .with_category("lsp")
                .with_description("Find references (LSP)"),
            KeybindingRegistration::new("K", lsp_nav::HOVER)
                .with_modes(&["vim:normal"])
                .with_category("lsp")
                .with_description("Show hover information (LSP)"),
            KeybindingRegistration::new("<C-k>", lsp_nav::SIGNATURE_HELP)
                .with_modes(&["vim:insert"])
                .with_category("lsp")
                .with_description("Show signature help (LSP)"),
        ]
    }
}

#[cfg(feature = "dynamic")]
reovim_module_macros::declare_module!(VimLspModule);

#[cfg(test)]
mod tests {
    use std::{path::PathBuf, sync::Arc};

    use super::*;

    #[test]
    fn module_id() {
        let module = VimLspModule::new();
        assert_eq!(module.id().as_str(), "vim-lsp");
    }

    #[test]
    fn module_name() {
        let module = VimLspModule::new();
        assert_eq!(module.name(), "Vim LSP Keybindings");
    }

    #[test]
    fn module_version() {
        let module = VimLspModule::new();
        let v = module.version();
        assert_eq!((v.major, v.minor, v.patch), (0, 1, 0));
    }

    #[test]
    #[allow(clippy::default_constructed_unit_structs)]
    fn module_default() {
        let module = VimLspModule::default();
        assert_eq!(module.id().as_str(), "vim-lsp");
    }

    #[test]
    fn module_exit() {
        let mut module = VimLspModule::new();
        assert!(module.exit().is_ok());
    }

    #[test]
    fn init_registers_keybindings() {
        use reovim_kernel::api::v1::ServiceRegistry;

        let services = Arc::new(ServiceRegistry::new());
        let ctx = test_module_context(services.clone());

        let mut module = VimLspModule::new();
        let result = module.init(&ctx);
        assert!(matches!(result, ProbeResult::Success));

        let store = services.get::<KeybindingStore>();
        assert!(store.is_some());
    }

    #[test]
    fn keybindings_count() {
        let module = VimLspModule::new();
        assert_eq!(module.keybindings().len(), 4);
    }

    #[test]
    fn keybindings_keys() {
        let module = VimLspModule::new();
        let keys: Vec<_> = module.keybindings().iter().map(|kb| kb.keys).collect();
        assert!(keys.contains(&"gd"));
        assert!(keys.contains(&"gr"));
        assert!(keys.contains(&"K"));
        assert!(keys.contains(&"<C-k>"));
    }

    #[test]
    fn keybindings_normal_mode() {
        let module = VimLspModule::new();
        let normal_keys: Vec<_> = module
            .keybindings()
            .into_iter()
            .filter(|kb| kb.modes.contains(&"vim:normal"))
            .map(|kb| kb.keys)
            .collect();
        assert!(normal_keys.contains(&"gd"));
        assert!(normal_keys.contains(&"gr"));
        assert!(normal_keys.contains(&"K"));
    }

    #[test]
    fn keybindings_insert_mode() {
        let module = VimLspModule::new();
        assert!(
            module
                .keybindings()
                .into_iter()
                .filter(|kb| kb.modes.contains(&"vim:insert"))
                .map(|kb| kb.keys)
                .any(|k| k == "<C-k>")
        );
    }

    #[test]
    fn keybindings_have_lsp_category() {
        let module = VimLspModule::new();
        for kb in module.keybindings() {
            assert_eq!(kb.category, Some("lsp"));
        }
    }

    #[test]
    fn keybindings_have_descriptions() {
        let module = VimLspModule::new();
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
