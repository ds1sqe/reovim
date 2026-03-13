#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! Indent guide rendering module for reovim.
//!
//! Provides vertical indent guide lines in leading whitespace. Uses
//! per-language indent configuration from [`IndentConfigStore`](reovim_driver_syntax::IndentConfigStore)
//! when available, falling back to pure whitespace analysis.

mod bridge;
pub mod state;

use {
    reovim_driver_session::bridges::BridgeProvider,
    reovim_kernel::api::v1::{Module, ModuleContext, ModuleError, ModuleId, ProbeResult, Version},
};

use crate::bridge::IndentGuideBridge;

/// Extension kind string.
const KIND: &str = "indent-guide";
/// Module ID constant.
const MODULE_ID: &str = "indent-guide";

/// Indent guide rendering module.
///
/// Registers [`IndentGuideBridge`] during `init()` so that guide state
/// is streamed to TUI/web clients via the extension bridge system.
pub struct IndentGuideModule;

impl IndentGuideModule {
    /// Create a new indent guide module.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Default for IndentGuideModule {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for IndentGuideModule {
    fn id(&self) -> ModuleId {
        ModuleId::new(MODULE_ID)
    }

    fn name(&self) -> &'static str {
        "Indent Guide"
    }

    fn version(&self) -> Version {
        Version::new(0, 1, 0)
    }

    fn init(&mut self, ctx: &ModuleContext) -> ProbeResult {
        let provider = ctx.services.get_or_create::<BridgeProvider>();
        provider.register(IndentGuideBridge);

        tracing::info!("IndentGuideModule: registered indent-guide extension bridge");
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
reovim_module_macros::declare_module!(IndentGuideModule);

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
