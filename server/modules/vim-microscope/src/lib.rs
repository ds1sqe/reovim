#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! Vim keybinding adapter for microscope picker commands.
//!
//! This adapter module bridges `vim` (editor personality) and
//! `microscope` (fuzzy finder). It maps vim normal-mode leader keys
//! to picker commands:
//! - `<Space>f` → Open file picker
//! - `<Space>b` → Open buffer picker
//! - `<Space>g` → Open grep picker
//! - `<Space>;` → Open command picker
//!
//! # Architecture
//!
//! This is an adapter (integration) module. It exists because
//! `microscope` should not know about vim's mode names:
//! - `microscope` provides commands and picker UI (no keybinding knowledge)
//! - `vim` provides modes and core keybindings (no picker knowledge)
//! - This module bridges them with explicit, visible coupling

use {
    reovim_driver_input::KeybindingStore,
    reovim_kernel::api::v1::{
        KeybindingRegistration, Module, ModuleContext, ModuleError, ModuleId, ProbeResult, Version,
    },
    reovim_module_microscope::ids as microscope,
};

const MODULE: ModuleId = ModuleId::new("vim-microscope");

/// Vim-microscope keybinding adapter module.
///
/// Registers picker opener keybindings in `vim:normal` mode,
/// targeting microscope commands defined in `microscope`.
pub struct VimMicroscopeModule;

impl VimMicroscopeModule {
    /// Create a new instance.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Default for VimMicroscopeModule {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for VimMicroscopeModule {
    fn id(&self) -> ModuleId {
        MODULE
    }

    fn name(&self) -> &'static str {
        "Vim Microscope Keybindings"
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
            KeybindingRegistration::new("<Space>f", microscope::OPEN_FILES)
                .with_modes(&["vim:normal"])
                .with_category("picker")
                .with_description("Open file picker"),
            KeybindingRegistration::new("<Space>b", microscope::OPEN_BUFFERS)
                .with_modes(&["vim:normal"])
                .with_category("picker")
                .with_description("Open buffer picker"),
            KeybindingRegistration::new("<Space>g", microscope::OPEN_GREP)
                .with_modes(&["vim:normal"])
                .with_category("picker")
                .with_description("Open grep picker"),
            KeybindingRegistration::new("<Space>;", microscope::OPEN_COMMANDS)
                .with_modes(&["vim:normal"])
                .with_category("picker")
                .with_description("Open command picker"),
        ]
    }
}

#[cfg(feature = "dynamic")]
reovim_module_macros::declare_module!(VimMicroscopeModule);

#[cfg(test)]
mod tests {
    use std::{path::PathBuf, sync::Arc};

    use super::*;

    #[test]
    fn module_id() {
        let module = VimMicroscopeModule::new();
        assert_eq!(module.id().as_str(), "vim-microscope");
    }

    #[test]
    fn module_name() {
        let module = VimMicroscopeModule::new();
        assert_eq!(module.name(), "Vim Microscope Keybindings");
    }

    #[test]
    fn module_version() {
        let module = VimMicroscopeModule::new();
        let v = module.version();
        assert_eq!((v.major, v.minor, v.patch), (0, 1, 0));
    }

    #[test]
    #[allow(clippy::default_constructed_unit_structs)]
    fn module_default() {
        let module = VimMicroscopeModule::default();
        assert_eq!(module.id().as_str(), "vim-microscope");
    }

    #[test]
    fn module_exit() {
        let mut module = VimMicroscopeModule::new();
        assert!(module.exit().is_ok());
    }

    #[test]
    fn init_registers_keybindings() {
        use reovim_kernel::api::v1::ServiceRegistry;

        let services = Arc::new(ServiceRegistry::new());
        let ctx = test_module_context(services.clone());

        let mut module = VimMicroscopeModule::new();
        let result = module.init(&ctx);
        assert!(matches!(result, ProbeResult::Success));

        let store = services.get::<KeybindingStore>();
        assert!(store.is_some());
    }

    #[test]
    fn keybindings_count() {
        let module = VimMicroscopeModule::new();
        assert_eq!(module.keybindings().len(), 4);
    }

    #[test]
    fn keybindings_keys() {
        let module = VimMicroscopeModule::new();
        let keys: Vec<_> = module.keybindings().iter().map(|kb| kb.keys).collect();
        assert!(keys.contains(&"<Space>f"));
        assert!(keys.contains(&"<Space>b"));
        assert!(keys.contains(&"<Space>g"));
        assert!(keys.contains(&"<Space>;"));
    }

    #[test]
    fn keybindings_target_vim_normal() {
        let module = VimMicroscopeModule::new();
        for kb in module.keybindings() {
            assert!(kb.modes.contains(&"vim:normal"));
        }
    }

    #[test]
    fn keybindings_have_picker_category() {
        let module = VimMicroscopeModule::new();
        for kb in module.keybindings() {
            assert_eq!(kb.category, Some("picker"));
        }
    }

    #[test]
    fn keybindings_have_descriptions() {
        let module = VimMicroscopeModule::new();
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
