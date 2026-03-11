#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! Vim keybinding adapter for completion commands.
//!
//! This adapter module bridges `vim` (editor personality) and
//! `completion` (completion engine). It maps vim insert-mode keys
//! to completion commands:
//! - `<C-y>` → Confirm completion
//! - `<C-e>` → Dismiss completion
//!
//! # Architecture
//!
//! This is an adapter (integration) module. It exists because
//! `completion` should not know about vim's mode names:
//! - `completion` provides commands and completion engine (no vim knowledge)
//! - `vim` provides modes and core keybindings (no completion knowledge)
//! - This module bridges them with explicit, visible coupling

use {
    reovim_driver_input::KeybindingStore,
    reovim_kernel::api::v1::{
        KeybindingRegistration, Module, ModuleContext, ModuleError, ModuleId, ProbeResult, Version,
    },
    reovim_module_completion::ids as completion,
};

const MODULE: ModuleId = ModuleId::new("vim-completion");

/// Vim-completion keybinding adapter module.
///
/// Registers completion confirm/dismiss keybindings in `vim:insert` mode,
/// targeting completion commands defined in `completion`.
pub struct VimCompletionModule;

impl VimCompletionModule {
    /// Create a new instance.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Default for VimCompletionModule {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for VimCompletionModule {
    fn id(&self) -> ModuleId {
        MODULE
    }

    fn name(&self) -> &'static str {
        "Vim Completion Keybindings"
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
            KeybindingRegistration::new("<C-y>", completion::CONFIRM)
                .with_modes(&["vim:insert"])
                .with_category("completion")
                .with_description("Confirm completion"),
            KeybindingRegistration::new("<C-e>", completion::DISMISS)
                .with_modes(&["vim:insert"])
                .with_category("completion")
                .with_description("Dismiss completion"),
        ]
    }
}

#[cfg(feature = "dynamic")]
reovim_module_macros::declare_module!(VimCompletionModule);

#[cfg(test)]
mod tests {
    use std::{path::PathBuf, sync::Arc};

    use super::*;

    #[test]
    fn module_id() {
        let module = VimCompletionModule::new();
        assert_eq!(module.id().as_str(), "vim-completion");
    }

    #[test]
    fn module_name() {
        let module = VimCompletionModule::new();
        assert_eq!(module.name(), "Vim Completion Keybindings");
    }

    #[test]
    fn module_version() {
        let module = VimCompletionModule::new();
        let v = module.version();
        assert_eq!((v.major, v.minor, v.patch), (0, 1, 0));
    }

    #[test]
    #[allow(clippy::default_constructed_unit_structs)]
    fn module_default() {
        let module = VimCompletionModule::default();
        assert_eq!(module.id().as_str(), "vim-completion");
    }

    #[test]
    fn module_exit() {
        let mut module = VimCompletionModule::new();
        assert!(module.exit().is_ok());
    }

    #[test]
    fn init_registers_keybindings() {
        use reovim_kernel::api::v1::ServiceRegistry;

        let services = Arc::new(ServiceRegistry::new());
        let ctx = test_module_context(services.clone());

        let mut module = VimCompletionModule::new();
        let result = module.init(&ctx);
        assert!(matches!(result, ProbeResult::Success));

        let store = services.get::<KeybindingStore>();
        assert!(store.is_some());
    }

    #[test]
    fn keybindings_count() {
        let module = VimCompletionModule::new();
        assert_eq!(module.keybindings().len(), 2);
    }

    #[test]
    fn keybindings_keys() {
        let module = VimCompletionModule::new();
        let keys: Vec<_> = module.keybindings().iter().map(|kb| kb.keys).collect();
        assert!(keys.contains(&"<C-y>"));
        assert!(keys.contains(&"<C-e>"));
    }

    #[test]
    fn keybindings_target_vim_insert() {
        let module = VimCompletionModule::new();
        for kb in module.keybindings() {
            assert!(kb.modes.contains(&"vim:insert"));
        }
    }

    #[test]
    fn keybindings_have_completion_category() {
        let module = VimCompletionModule::new();
        for kb in module.keybindings() {
            assert_eq!(kb.category, Some("completion"));
        }
    }

    #[test]
    fn keybindings_have_descriptions() {
        let module = VimCompletionModule::new();
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
