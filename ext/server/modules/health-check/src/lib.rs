#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! Health-check diagnostic command module - POLICY.
//!
//! Implements `:checkhealth` for reporting system status, installed tools,
//! and configuration validation.

mod command;
mod diagnostics;

use {
    reovim_driver_command::CommandHandlerStore,
    reovim_kernel::api::v1::{Module, ModuleContext, ModuleError, ModuleId, ProbeResult, Version},
};

pub use command::CheckHealthCommand;

/// Health-check module instance.
pub struct HealthCheckModule;

impl HealthCheckModule {
    /// Create a new health-check module.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Default for HealthCheckModule {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for HealthCheckModule {
    fn id(&self) -> ModuleId {
        ModuleId::new("health-check")
    }

    fn name(&self) -> &'static str {
        "Health Check"
    }

    fn version(&self) -> Version {
        Version::new(0, 1, 0)
    }

    fn init(&mut self, ctx: &ModuleContext) -> ProbeResult {
        let store = ctx.services.get_or_create::<CommandHandlerStore>();
        store.add(Box::new(CheckHealthCommand::new()));
        ProbeResult::Success
    }

    fn exit(&mut self) -> Result<(), ModuleError> {
        Ok(())
    }
}

#[cfg(feature = "dynamic")]
reovim_module_macros::declare_module!(HealthCheckModule);

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
