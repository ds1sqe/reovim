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
            KeybindingRegistration::new("<Space>o", microscope::OPEN_OPTIONS)
                .with_modes(&["vim:normal"])
                .with_category("picker")
                .with_description("Open option picker"),
        ]
    }
}

#[cfg(feature = "dynamic")]
reovim_module_macros::declare_module!(VimMicroscopeModule);

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
