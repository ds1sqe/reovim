#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! Command-line mode module - POLICY layer.
//!
//! This module owns all command-line mode state and behavior:
//! - [`CmdlineState`] - per-client session extension
//! - [`CmdlinePrompt`] - prompt type enum (`:`, `/`, `?`)
//! - [`CmdlineBridge`] - adapts state to JSON for gRPC
//!
//! # Architecture (#468)
//!
//! `CmdlineState` was moved here from the driver layer because it is POLICY
//! (defines HOW command-line mode behaves). The driver layer only provides
//! the trait contracts (MECHANISM).
//!
//! The bridge is registered via `BridgeProvider` during `init()`, enabling
//! generic bridge detection without hardcoded server/driver code.

mod bridge;
mod state;

pub use {
    bridge::CmdlineBridge,
    state::{CmdlineMessage, CmdlinePrompt, CmdlineState},
};

use {
    reovim_driver_session::bridges::BridgeProvider,
    reovim_kernel::api::v1::{Module, ModuleContext, ModuleError, ModuleId, ProbeResult, Version},
};

const KIND: &str = "cmdline";
const MODULE_ID: ModuleId = ModuleId::new("cmdline");

/// Command-line mode module.
///
/// Registers `CmdlineBridge` via `BridgeProvider` during `init()`.
pub struct CmdlineModule;

impl CmdlineModule {
    /// Create a new instance.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Default for CmdlineModule {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for CmdlineModule {
    fn id(&self) -> ModuleId {
        MODULE_ID
    }

    fn name(&self) -> &'static str {
        "cmdline"
    }

    fn version(&self) -> Version {
        Version::new(0, 1, 0)
    }

    fn init(&mut self, ctx: &ModuleContext) -> ProbeResult {
        // Register CmdlineBridge via BridgeProvider (#468)
        let provider = ctx.services.get_or_create::<BridgeProvider>();
        provider.register(CmdlineBridge);
        ProbeResult::Success
    }

    fn exit(&mut self) -> Result<(), ModuleError> {
        Ok(())
    }

    fn extension_kinds(&self) -> &[&'static str] {
        &[KIND]
    }
}

#[cfg(feature = "dynamic")]
reovim_module_macros::declare_module!(CmdlineModule);

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
