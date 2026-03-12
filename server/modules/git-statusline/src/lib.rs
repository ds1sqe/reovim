#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! Git statusline module for reovim.
//!
//! Provides a `ComponentProvider` that renders the current git branch
//! name for the statusline. Registers into `ComponentProviderRegistry`
//! so the statusline provider can display it.

use std::sync::Arc;

use {
    reovim_driver_display::statusline::{ComponentProviderKey, ComponentProviderRegistry},
    reovim_driver_git::GitProviderStore,
    reovim_kernel::api::v1::{Module, ModuleContext, ModuleError, ModuleId, ProbeResult, Version},
};

mod branch;

pub use branch::BranchComponent;

/// Git statusline module.
///
/// Registers a `BranchComponent` into the `ComponentProviderRegistry`
/// under the key `"branch"`.
pub struct GitStatuslineModule;

impl GitStatuslineModule {
    /// Create a new instance.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Default for GitStatuslineModule {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for GitStatuslineModule {
    fn id(&self) -> ModuleId {
        ModuleId::new("git-statusline")
    }

    fn name(&self) -> &'static str {
        "Git Statusline"
    }

    fn version(&self) -> Version {
        Version::new(0, 1, 0)
    }

    fn init(&mut self, ctx: &ModuleContext) -> ProbeResult {
        let Some(store) = ctx.services.get::<GitProviderStore>() else {
            return ProbeResult::Success;
        };
        let Some(provider) = store.get() else {
            return ProbeResult::Success;
        };

        let registry = ctx.services.get_or_create::<ComponentProviderRegistry>();
        registry.register(
            ComponentProviderKey::new("branch"),
            Arc::new(BranchComponent::new(provider)),
        );

        ProbeResult::Success
    }

    fn exit(&mut self) -> Result<(), ModuleError> {
        Ok(())
    }
}

#[cfg(feature = "dynamic")]
reovim_module_macros::declare_module!(GitStatuslineModule);

#[cfg(test)]
mod tests;
