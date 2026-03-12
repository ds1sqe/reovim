#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! Git log picker module for reovim.
//!
//! Lists commit history via [`GitProvider`](reovim_driver_git::GitProvider).
//! Supports query-based filtering (dynamic picker).

use std::sync::Arc;

use {
    reovim_driver_git::GitProviderStore,
    reovim_driver_picker::{
        Picker, PickerAction, PickerContext, PickerData, PickerItem, PickerRegistry,
    },
    reovim_kernel::api::v1::{
        Module, ModuleContext, ModuleError, ModuleId, ProbeResult, ServiceRegistry, Version,
    },
};

/// Maximum number of log entries to fetch.
const LOG_LIMIT: usize = 200;

/// Picker that lists git commit log.
///
/// This is a dynamic picker (`is_static() = false`) — it re-fetches
/// results when the query changes, filtering commits by message.
pub struct GitLogPicker;

impl GitLogPicker {
    /// Create a new instance.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Default for GitLogPicker {
    fn default() -> Self {
        Self::new()
    }
}

impl Picker for GitLogPicker {
    fn name(&self) -> &'static str {
        "git-log"
    }

    fn title(&self) -> &'static str {
        "Git Log"
    }

    fn is_static(&self) -> bool {
        false
    }

    fn items(&self, ctx: &PickerContext, services: &ServiceRegistry) -> Vec<PickerItem> {
        let Some(store) = services.get::<GitProviderStore>() else {
            return vec![];
        };
        let Some(git) = store.get() else {
            return vec![];
        };

        git.log(&ctx.cwd, &ctx.query, LOG_LIMIT)
            .into_iter()
            .map(|entry| PickerItem {
                display: format!("{} {}", entry.short_hash, entry.message),
                detail: Some(format!("{} — {}", entry.author, entry.date)),
                data: PickerData::Text(entry.hash),
                icon: None,
            })
            .collect()
    }

    fn on_select(&self, _item: &PickerItem) -> PickerAction {
        PickerAction::Close
    }
}

/// Git log picker module.
pub struct PickerGitLogModule;

impl PickerGitLogModule {
    /// Create a new instance.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Default for PickerGitLogModule {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for PickerGitLogModule {
    fn id(&self) -> ModuleId {
        ModuleId::new("picker-git-log")
    }

    fn name(&self) -> &'static str {
        "Git Log Picker"
    }

    fn version(&self) -> Version {
        Version::new(0, 1, 0)
    }

    fn init(&mut self, ctx: &ModuleContext) -> ProbeResult {
        let registry = ctx.services.get_or_create::<PickerRegistry>();
        registry.register(Arc::new(GitLogPicker));
        ProbeResult::Success
    }

    fn exit(&mut self) -> Result<(), ModuleError> {
        Ok(())
    }
}

#[cfg(feature = "dynamic")]
reovim_module_macros::declare_module!(PickerGitLogModule);

#[cfg(test)]
mod tests;
