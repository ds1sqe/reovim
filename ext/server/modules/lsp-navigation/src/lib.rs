#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! LSP navigation module for reovim.
//!
//! Provides `gd` (goto definition) and `gr` (find references) commands.
//! Single definition results jump directly; multiple results and all
//! references open in the microscope picker.

pub mod commands;
pub mod diagnostic_nav;
pub mod hover_bridge;
pub mod hover_state;
pub mod ids;
mod keybinding;
mod picker;
pub mod signature_help_bridge;
pub mod signature_help_state;

use std::sync::Arc;

use {
    reovim_driver_command::CommandHandlerStore,
    reovim_driver_picker::PickerRegistry,
    reovim_driver_text_input::KeybindingStore,
    reovim_driver_text_session::{TickSchedulerHandle, bridges::BridgeProvider},
    reovim_kernel::api::v1::{
        KeybindingRegistration, Module, ModuleContext, ModuleError, ModuleId, ProbeResult, Version,
    },
};

use picker::LspLocationPicker;

pub(crate) const KIND_HOVER: &str = "hover";
pub(crate) const KIND_SIGNATURE_HELP: &str = "signature-help";

/// LSP navigation module.
///
/// Registers command handlers for `gd` and `gr`, and the
/// `lsp-locations` picker in `PickerRegistry`.
pub struct LspNavigationModule;

impl LspNavigationModule {
    /// Create a new instance.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Default for LspNavigationModule {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for LspNavigationModule {
    fn id(&self) -> ModuleId {
        ids::MODULE
    }

    fn name(&self) -> &'static str {
        "LSP Navigation"
    }

    fn version(&self) -> Version {
        Version::new(0, 1, 0)
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn init(&mut self, ctx: &ModuleContext) -> ProbeResult {
        // Register the lsp-locations picker.
        let picker_registry = ctx.services.get_or_create::<PickerRegistry>();
        picker_registry.register(Arc::new(LspLocationPicker::new()));

        // Register command handlers.
        let command_store = ctx.services.get_or_create::<CommandHandlerStore>();
        for handler in commands::command_handlers() {
            command_store.add(handler);
        }
        for handler in diagnostic_nav::command_handlers() {
            command_store.add(handler);
        }

        // Register extension bridges for state serialization.
        let bridge_provider = ctx.services.get_or_create::<BridgeProvider>();
        bridge_provider.register(hover_bridge::HoverBridge);
        bridge_provider.register(signature_help_bridge::SignatureHelpBridge);

        // Ensure HoverCache exists for async hover pipeline (#662).
        let _ = ctx.services.get_or_create::<hover_state::HoverCache>();

        // Ensure TickSchedulerHandle exists for hover tick (#662).
        let _ = ctx.services.get_or_create::<TickSchedulerHandle>();

        // #700: Register keybindings from personality adapters
        let keybinding_store = ctx.services.get_or_create::<KeybindingStore>();
        keybinding_store.add_all(self.keybindings());

        ProbeResult::Success
    }

    fn exit(&mut self) -> Result<(), ModuleError> {
        Ok(())
    }

    fn extension_kinds(&self) -> &[&'static str] {
        &[KIND_HOVER, KIND_SIGNATURE_HELP]
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn keybindings(&self) -> Vec<KeybindingRegistration> {
        keybinding::all()
    }
}

#[cfg(feature = "dynamic")]
reovim_module_macros::declare_module!(LspNavigationModule);

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
