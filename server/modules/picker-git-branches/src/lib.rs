#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! Git branches picker module for reovim.
//!
//! Lists local branches via [`GitProvider`](reovim_driver_git::GitProvider)
//! and registers in the `PickerRegistry` for use with microscope.

use std::sync::Arc;

use {
    reovim_driver_picker::{
        Picker, PickerAction, PickerContext, PickerData, PickerItem, PickerRegistry,
    },
    reovim_kernel::api::v1::{
        Module, ModuleContext, ModuleError, ModuleId, ProbeResult, ServiceRegistry, Version,
    },
    reovim_driver_git::GitProviderStore,
};

/// Picker that lists git branches.
///
/// Uses [`GitProviderStore`] from `ServiceRegistry` to fetch branch data.
/// The current branch is marked with a `*` icon.
pub struct GitBranchesPicker;

impl GitBranchesPicker {
    /// Create a new instance.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Default for GitBranchesPicker {
    fn default() -> Self {
        Self::new()
    }
}

impl Picker for GitBranchesPicker {
    fn name(&self) -> &'static str {
        "git-branches"
    }

    fn title(&self) -> &'static str {
        "Git Branches"
    }

    fn items(&self, ctx: &PickerContext, services: &ServiceRegistry) -> Vec<PickerItem> {
        let Some(store) = services.get::<GitProviderStore>() else {
            return vec![];
        };
        let Some(git) = store.get() else {
            return vec![];
        };

        git.branches(&ctx.cwd)
            .into_iter()
            .map(|branch| {
                let icon = if branch.is_current { Some('*') } else { None };
                PickerItem {
                    display: branch.name.clone(),
                    detail: branch.upstream,
                    data: PickerData::Text(branch.name),
                    icon,
                }
            })
            .collect()
    }

    fn on_select(&self, _item: &PickerItem) -> PickerAction {
        PickerAction::Close
    }
}

/// Git branches picker module.
pub struct PickerGitBranchesModule;

impl PickerGitBranchesModule {
    /// Create a new instance.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Default for PickerGitBranchesModule {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for PickerGitBranchesModule {
    fn id(&self) -> ModuleId {
        ModuleId::new("picker-git-branches")
    }

    fn name(&self) -> &'static str {
        "Git Branches Picker"
    }

    fn version(&self) -> Version {
        Version::new(0, 1, 0)
    }

    fn init(&mut self, ctx: &ModuleContext) -> ProbeResult {
        let registry = ctx.services.get_or_create::<PickerRegistry>();
        registry.register(Arc::new(GitBranchesPicker));
        ProbeResult::Success
    }

    fn exit(&mut self) -> Result<(), ModuleError> {
        Ok(())
    }
}

#[cfg(feature = "dynamic")]
reovim_module_macros::declare_module!(PickerGitBranchesModule);

#[cfg(test)]
mod tests;
