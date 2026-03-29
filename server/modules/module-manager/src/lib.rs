#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! Module manager module - POLICY (#622).
//!
//! Provides `:Modules` command and interactive module management panel.
//! Uses the extension bridge pattern (same as microscope) to push
//! per-client state to TUI/web clients.

mod bridge;
mod command;
mod commands;
pub mod ids;
pub mod modes;
mod resolver;
pub mod state;

use {
    reovim_driver_command::CommandHandlerStore,
    reovim_driver_input::{KeybindingStore, ModeInfo, ModeInfoStore, ResolverRegistry},
    reovim_driver_session::bridges::BridgeProvider,
    reovim_kernel::api::v1::{
        KeybindingRegistration, Module, ModuleContext, ModuleError, ModuleId, ProbeResult, Version,
    },
};

pub use {bridge::ModuleManagerBridge, command::ModulesCommand, state::ModuleManagerState};

const KIND: &str = "module-manager";

/// Module manager module instance.
pub struct ModuleManagerModule;

impl ModuleManagerModule {
    /// Create a new module manager module.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Default for ModuleManagerModule {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for ModuleManagerModule {
    fn id(&self) -> ModuleId {
        ModuleId::new("module-manager")
    }

    fn name(&self) -> &'static str {
        "Module Manager"
    }

    fn version(&self) -> Version {
        Version::new(0, 1, 0)
    }

    fn extension_kinds(&self) -> &[&'static str] {
        &[KIND]
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn keybindings(&self) -> Vec<KeybindingRegistration> {
        vec![
            KeybindingRegistration::new("j", ids::NEXT)
                .with_modes(&["module-manager:MANAGER"])
                .with_description("Next module"),
            KeybindingRegistration::new("<Down>", ids::NEXT)
                .with_modes(&["module-manager:MANAGER"])
                .with_description("Next module"),
            KeybindingRegistration::new("k", ids::PREV)
                .with_modes(&["module-manager:MANAGER"])
                .with_description("Previous module"),
            KeybindingRegistration::new("<Up>", ids::PREV)
                .with_modes(&["module-manager:MANAGER"])
                .with_description("Previous module"),
            KeybindingRegistration::new("<CR>", ids::TOGGLE_DETAIL)
                .with_modes(&["module-manager:MANAGER"])
                .with_description("Toggle detail"),
            KeybindingRegistration::new("<Tab>", ids::TOGGLE_FILTER)
                .with_modes(&["module-manager:MANAGER"])
                .with_description("Cycle filter"),
            KeybindingRegistration::new("q", ids::CLOSE)
                .with_modes(&["module-manager:MANAGER"])
                .with_description("Close"),
            KeybindingRegistration::new("<Esc>", ids::CLOSE)
                .with_modes(&["module-manager:MANAGER"])
                .with_description("Close"),
        ]
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn init(&mut self, ctx: &ModuleContext) -> ProbeResult {
        // Register ModuleManagerBridge via BridgeProvider
        let provider = ctx.services.get_or_create::<BridgeProvider>();
        provider.register(ModuleManagerBridge);

        // Register modes
        let mode_store = ctx.services.get_or_create::<ModeInfoStore>();
        for mode in modes::ManagerMode::ALL {
            mode_store.add(ModeInfo::from_mode(*mode));
        }

        // Register resolver for module-manager:MANAGER mode
        let resolver_registry = ctx.services.get_or_create::<ResolverRegistry>();
        resolver_registry.register(resolver::ManagerResolver::new());

        // Register :Modules ex-command
        let command_store = ctx.services.get_or_create::<CommandHandlerStore>();
        command_store.add(Box::new(ModulesCommand::new()));

        // Register navigation command handlers
        for handler in commands::command_handlers() {
            command_store.add(handler);
        }

        // Register keybindings for module-manager:MANAGER mode
        let keybinding_store = ctx.services.get_or_create::<KeybindingStore>();
        keybinding_store.add_all(self.keybindings());

        ProbeResult::Success
    }

    fn exit(&mut self) -> Result<(), ModuleError> {
        Ok(())
    }
}

#[cfg(feature = "dynamic")]
reovim_module_macros::declare_module!(ModuleManagerModule);

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
