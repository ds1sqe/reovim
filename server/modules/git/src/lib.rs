#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! Git provider module for reovim — POLICY layer.
//!
//! Registers a subprocess-based [`GitProvider`](reovim_driver_git::GitProvider)
//! into the `ServiceRegistry` during initialization. Any module that needs
//! git data (pickers, statusline, gutter, blame) can retrieve the provider
//! via `GitProviderStore`.
//!
//! # Architecture (#530)
//!
//! This module owns the HOW (subprocess `git` CLI). The driver crate
//! (`reovim-driver-git`) owns the WHAT (trait + typed data structs).

pub mod subprocess;

use {
    reovim_driver_git::GitProviderStore,
    reovim_kernel::api::v1::{Module, ModuleContext, ModuleError, ModuleId, ProbeResult, Version},
    std::sync::Arc,
    subprocess::SubprocessGitProvider,
};

/// Git provider module.
///
/// Registers [`SubprocessGitProvider`] during `init()` so other modules
/// can query git data via [`GitProviderStore`].
pub struct GitModule;

impl GitModule {
    /// Create a new instance.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Default for GitModule {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for GitModule {
    fn id(&self) -> ModuleId {
        ModuleId::new("git")
    }

    fn name(&self) -> &'static str {
        "Git Provider"
    }

    fn version(&self) -> Version {
        Version::new(0, 1, 0)
    }

    fn init(&mut self, ctx: &ModuleContext) -> ProbeResult {
        let store = ctx.services.get_or_create::<GitProviderStore>();
        store.register(Arc::new(SubprocessGitProvider::new()));
        ProbeResult::Success
    }

    fn exit(&mut self) -> Result<(), ModuleError> {
        Ok(())
    }
}

#[cfg(feature = "dynamic")]
reovim_module_macros::declare_module!(GitModule);

#[cfg(test)]
mod tests;

#[cfg(test)]
#[path = "subprocess_tests.rs"]
mod subprocess_tests;
