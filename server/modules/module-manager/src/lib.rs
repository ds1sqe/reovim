#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! Module manager command module - POLICY.
//!
//! Implements `:Modules` for listing loaded, disabled, and failed modules.
//! LazyVim-style module status overview (#622).

mod command;

use {
    reovim_driver_command::CommandHandlerStore,
    reovim_kernel::api::v1::{Module, ModuleContext, ModuleError, ModuleId, ProbeResult, Version},
};

pub use command::ModulesCommand;

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

    fn init(&mut self, ctx: &ModuleContext) -> ProbeResult {
        let store = ctx.services.get_or_create::<CommandHandlerStore>();
        store.add(Box::new(ModulesCommand::new()));
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
