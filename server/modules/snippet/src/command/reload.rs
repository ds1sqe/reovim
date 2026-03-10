//! `ReloadSnippets` command handler (#529).
//!
//! Re-reads all snippet files from disk and replaces the active registry
//! without restarting the editor.

use std::path::PathBuf;

use {
    reovim_driver_command::{Command, CommandContext, CommandHandler, CommandResult},
    reovim_driver_session::{
        NotificationDrainRegistry, PendingLevel, PendingNotificationQueue, SessionRuntime,
    },
    reovim_kernel::api::v1::CommandId,
};

use crate::{ids, provider::SnippetRegistryHandle};

/// Reload all snippet files from disk.
///
/// Rebuilds the snippet registry from the module data directory and
/// atomically swaps it into the shared handle. Running snippet sessions
/// are not affected (they hold cloned definitions).
pub struct ReloadSnippets {
    handle: SnippetRegistryHandle,
    data_dir: PathBuf,
}

impl ReloadSnippets {
    /// Create a new reload command.
    #[must_use]
    pub const fn new(handle: SnippetRegistryHandle, data_dir: PathBuf) -> Self {
        Self { handle, data_dir }
    }
}

impl Command for ReloadSnippets {
    fn id(&self) -> CommandId {
        ids::RELOAD
    }

    fn description(&self) -> &'static str {
        "Reload snippet files"
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl CommandHandler for ReloadSnippets {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        let new_registry = crate::build_registry(&self.data_dir);
        let count = new_registry.provider_count();
        self.handle.replace(new_registry);

        if let Some(queue) = runtime.kernel().services.get::<PendingNotificationQueue>() {
            queue.push(PendingLevel::Info, format!("Snippets reloaded ({count} sources)"));
        }

        // Drain immediately so notification appears in this command cycle.
        let drain = runtime
            .kernel()
            .services
            .get::<NotificationDrainRegistry>()
            .and_then(|reg| reg.get());
        if let Some(d) = drain {
            d.drain_pending(runtime);
        }

        CommandResult::Success
    }
}

#[cfg(test)]
mod tests {
    use crate::provider::SnippetRegistry;

    use super::*;

    #[test]
    fn test_reload_command_id() {
        let handle = SnippetRegistryHandle::new(SnippetRegistry::new());
        let cmd = ReloadSnippets::new(handle, PathBuf::from("/tmp"));
        assert_eq!(cmd.id(), ids::RELOAD);
    }

    #[test]
    fn test_reload_description() {
        let handle = SnippetRegistryHandle::new(SnippetRegistry::new());
        let cmd = ReloadSnippets::new(handle, PathBuf::from("/tmp"));
        assert!(!cmd.description().is_empty());
    }
}
