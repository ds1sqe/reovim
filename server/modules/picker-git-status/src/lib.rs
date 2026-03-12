#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! Git status picker module for reovim.
//!
//! Lists working tree changes via [`GitProvider`](reovim_driver_git::GitProvider)
//! and opens the selected file on confirmation.

use std::sync::Arc;

use {
    reovim_driver_git::{GitProviderStore, types::FileStatus},
    reovim_driver_picker::{
        Picker, PickerAction, PickerContext, PickerData, PickerItem, PickerRegistry, SessionRuntime,
    },
    reovim_driver_session::{BufferApi, WindowApi},
    reovim_kernel::api::v1::{
        Module, ModuleContext, ModuleError, ModuleId, ProbeResult, ServiceRegistry, Version,
    },
};

/// Picker that lists git status entries.
///
/// Shows modified, added, deleted, and untracked files from the
/// working tree. Selecting a file opens it in the editor.
pub struct GitStatusPicker;

impl GitStatusPicker {
    /// Create a new instance.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Default for GitStatusPicker {
    fn default() -> Self {
        Self::new()
    }
}

impl Picker for GitStatusPicker {
    fn name(&self) -> &'static str {
        "git-status"
    }

    fn title(&self) -> &'static str {
        "Git Status"
    }

    fn items(&self, ctx: &PickerContext, services: &ServiceRegistry) -> Vec<PickerItem> {
        let Some(store) = services.get::<GitProviderStore>() else {
            return vec![];
        };
        let Some(git) = store.get() else {
            return vec![];
        };

        git.status(&ctx.cwd)
            .into_iter()
            .map(|entry| {
                let status_str = format!(
                    "{}{}",
                    status_char(entry.index_status),
                    status_char(entry.worktree_status),
                );
                let full_path = ctx.cwd.join(&entry.path);

                PickerItem {
                    display: entry.path.to_string_lossy().to_string(),
                    detail: Some(status_str),
                    data: PickerData::FilePath(full_path),
                    icon: status_icon(entry.index_status, entry.worktree_status),
                }
            })
            .collect()
    }

    fn on_select(&self, item: &PickerItem) -> PickerAction {
        match &item.data {
            PickerData::FilePath(path) => PickerAction::OpenFile(path.clone()),
            _ => PickerAction::Close,
        }
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn execute(&self, action: PickerAction, runtime: &mut SessionRuntime<'_>) {
        if let PickerAction::OpenFile(path) = action
            && let Ok(content) = std::fs::read_to_string(&path)
        {
            let buf = runtime.create_buffer(Some(&path.to_string_lossy()), &content);
            runtime.set_active_buffer(Some(buf));
            if let Some(win) = runtime.active_window() {
                let _ = runtime.set_window_buffer(win, buf);
            }
        }
    }
}

/// Convert a `FileStatus` to a single display character.
const fn status_char(status: FileStatus) -> char {
    match status {
        FileStatus::Modified => 'M',
        FileStatus::Added => 'A',
        FileStatus::Deleted => 'D',
        FileStatus::Renamed => 'R',
        FileStatus::Copied => 'C',
        FileStatus::Untracked => '?',
        FileStatus::Ignored => '!',
        FileStatus::Unmodified => ' ',
    }
}

/// Pick an icon based on the combined status.
const fn status_icon(index: FileStatus, worktree: FileStatus) -> Option<char> {
    match (index, worktree) {
        (FileStatus::Added, _) | (_, FileStatus::Added) => Some('+'),
        (FileStatus::Deleted, _) | (_, FileStatus::Deleted) => Some('-'),
        (FileStatus::Modified, _) | (_, FileStatus::Modified) => Some('~'),
        (FileStatus::Untracked, _) | (_, FileStatus::Untracked) => Some('?'),
        _ => None,
    }
}

/// Git status picker module.
pub struct PickerGitStatusModule;

impl PickerGitStatusModule {
    /// Create a new instance.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Default for PickerGitStatusModule {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for PickerGitStatusModule {
    fn id(&self) -> ModuleId {
        ModuleId::new("picker-git-status")
    }

    fn name(&self) -> &'static str {
        "Git Status Picker"
    }

    fn version(&self) -> Version {
        Version::new(0, 1, 0)
    }

    fn init(&mut self, ctx: &ModuleContext) -> ProbeResult {
        let registry = ctx.services.get_or_create::<PickerRegistry>();
        registry.register(Arc::new(GitStatusPicker));
        ProbeResult::Success
    }

    fn exit(&mut self) -> Result<(), ModuleError> {
        Ok(())
    }
}

#[cfg(feature = "dynamic")]
reovim_module_macros::declare_module!(PickerGitStatusModule);

#[cfg(test)]
mod tests;
