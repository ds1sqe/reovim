//! `SnippetCatalog` command handler (#529).
//!
//! Lists all available snippets for the current buffer's filetype
//! via a notification toast.

use {
    reovim_driver_command::{Command, CommandContext, CommandHandler, CommandResult},
    reovim_driver_session::{
        BufferApi, NotificationDrainRegistry, PendingLevel, PendingNotificationQueue,
        SessionRuntime,
    },
    reovim_kernel::api::v1::CommandId,
};

use crate::{ids, provider::SnippetRegistryHandle};

/// List available snippets for the current filetype.
///
/// Collects all snippets from the registry for the buffer's filetype
/// and shows them via notification toast.
pub struct SnippetCatalog {
    handle: SnippetRegistryHandle,
}

impl SnippetCatalog {
    /// Create a new catalog command with access to the snippet registry.
    #[must_use]
    pub const fn new(handle: SnippetRegistryHandle) -> Self {
        Self { handle }
    }
}

impl Command for SnippetCatalog {
    fn id(&self) -> CommandId {
        ids::CATALOG
    }

    fn description(&self) -> &'static str {
        "List available snippets"
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl CommandHandler for SnippetCatalog {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        let Some(buffer_id) = args.buffer_id() else {
            return CommandResult::error("no active buffer");
        };

        let filetype = runtime
            .buffer_file_path(buffer_id)
            .as_deref()
            .map(reovim_driver_vfs::filetype_id)
            .filter(|ft| !ft.is_empty())
            .unwrap_or("global");

        let snippets = self.handle.all_for_filetype(filetype);

        if snippets.is_empty() {
            if let Some(queue) = runtime.kernel().services.get::<PendingNotificationQueue>() {
                queue.push(PendingLevel::Info, format!("No snippets for {filetype}"));
            }
            drain_notifications(runtime);
            return CommandResult::Success;
        }

        // Format as table: prefix → description/name
        let mut lines = Vec::with_capacity(snippets.len());
        for def in &snippets {
            let desc = def.description.as_deref().unwrap_or(def.name.as_str());
            lines.push(format!("  {:<12} {desc}", def.prefix));
        }
        let body = lines.join("\n");

        let title = format!("Snippets for {filetype} ({} available)\n{body}", snippets.len());

        if let Some(queue) = runtime.kernel().services.get::<PendingNotificationQueue>() {
            queue.push(PendingLevel::Info, title);
        }
        drain_notifications(runtime);

        CommandResult::Success
    }
}

/// Drain pending notifications so they appear immediately in this command cycle.
#[cfg_attr(coverage_nightly, coverage(off))]
fn drain_notifications(runtime: &mut SessionRuntime<'_>) {
    let drain = runtime
        .kernel()
        .services
        .get::<NotificationDrainRegistry>()
        .and_then(|reg| reg.get());
    if let Some(d) = drain {
        d.drain_pending(runtime);
    }
}

#[cfg(test)]
mod tests {
    use crate::provider::SnippetRegistry;

    use super::*;

    #[test]
    fn test_catalog_command_id() {
        let cmd = SnippetCatalog::new(SnippetRegistryHandle::new(SnippetRegistry::new()));
        assert_eq!(cmd.id(), ids::CATALOG);
    }

    #[test]
    fn test_catalog_description() {
        let cmd = SnippetCatalog::new(SnippetRegistryHandle::new(SnippetRegistry::new()));
        assert!(!cmd.description().is_empty());
    }
}
