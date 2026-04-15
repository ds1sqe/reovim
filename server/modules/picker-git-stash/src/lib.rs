#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! Git stash picker module for reovim.
//!
//! Lists stash entries via [`GitProvider`](reovim_driver_git::GitProvider).

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

/// Picker that lists git stash entries.
pub struct GitStashPicker;

impl GitStashPicker {
    /// Create a new instance.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Default for GitStashPicker {
    fn default() -> Self {
        Self::new()
    }
}

impl Picker for GitStashPicker {
    fn name(&self) -> &'static str {
        "git-stash"
    }

    fn title(&self) -> &'static str {
        "Git Stash"
    }

    fn items(&self, ctx: &PickerContext, services: &ServiceRegistry) -> Vec<PickerItem> {
        let Some(store) = services.get::<GitProviderStore>() else {
            return vec![];
        };
        let Some(git) = store.get() else {
            return vec![];
        };

        git.stash_list(&ctx.cwd)
            .into_iter()
            .map(|entry| PickerItem {
                display: entry.message,
                detail: Some(format!("stash@{{{}}}", entry.index)),
                data: PickerData::Text(format!("stash@{{{}}}", entry.index)),
                icon: None,
            })
            .collect()
    }

    fn on_select(&self, _item: &PickerItem) -> PickerAction {
        PickerAction::Close
    }
}

/// Git stash picker module.
pub struct PickerGitStashModule;

impl PickerGitStashModule {
    /// Create a new instance.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Default for PickerGitStashModule {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for PickerGitStashModule {
    fn id(&self) -> ModuleId {
        ModuleId::new("picker-git-stash")
    }

    fn name(&self) -> &'static str {
        "Git Stash Picker"
    }

    fn version(&self) -> Version {
        Version::new(0, 1, 0)
    }

    fn init(&mut self, ctx: &ModuleContext) -> ProbeResult {
        let registry = ctx.services.get_or_create::<PickerRegistry>();
        registry.register(Arc::new(GitStashPicker));
        ProbeResult::Success
    }

    fn exit(&mut self) -> Result<(), ModuleError> {
        Ok(())
    }
}

#[cfg(feature = "dynamic")]
reovim_module_macros::declare_module!(PickerGitStashModule);

#[cfg(test)]
mod tests;
