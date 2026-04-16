//! Command registry for storing and querying command metadata.
//!
//! Commands are stored by their [`CommandId`] as `Arc<dyn Command>` (subsys trait,
//! metadata-only). Command execution routes through `DomainDriver` — the server
//! never calls command handlers directly (#753 E6).
//!
//! # Module Ownership
//!
//! Commands can be registered with optional module ownership via
//! [`CommandRegistry::register_for_module`]. When a module is unloaded, all its
//! registered commands can be removed via [`CommandRegistry::unregister_for_module`].

use std::{collections::HashMap, sync::Arc};

use {
    reovim_kernel::api::v1::{CommandId, ModuleId, Service},
    reovim_subsys_command::{Command, CommandInfo, CommandPriority, CommandQueryService},
};

/// Entry in the command registry with optional ownership tracking.
#[derive(Clone)]
struct CommandEntry {
    /// The command (metadata-only, execution via DomainDriver).
    handler: Arc<dyn Command>,
    /// The module that owns this command (if any).
    owner: Option<ModuleId>,
    /// Registration priority (#545). Higher priority wins on conflict.
    priority: CommandPriority,
}

/// Registry for command metadata.
///
/// Stores [`Command`] implementations keyed by [`CommandId`].
/// The server uses this for metadata queries (names, descriptions, args).
/// Command execution routes through `DomainDriver` (#753 E6).
///
/// # Module Ownership
///
/// Commands can be registered with module ownership via [`Self::register_for_module`].
/// This enables automatic cleanup when modules are unloaded.
#[derive(Default, Clone)]
pub struct CommandRegistry {
    entries: HashMap<CommandId, CommandEntry>,
}

impl CommandRegistry {
    /// Create a new empty command registry.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a command (without module ownership).
    ///
    /// The command's ID is obtained via its `id()` method.
    /// If a command with the same ID already exists, the higher priority
    /// handler wins. Equal priority uses last-wins semantics (#545).
    pub fn register(&mut self, handler: Arc<dyn Command>) {
        let id = handler.id();
        let new_priority = handler.priority();

        if self
            .entries
            .get(&id)
            .is_some_and(|existing| new_priority < existing.priority)
        {
            return;
        }

        self.entries.insert(
            id,
            CommandEntry {
                handler,
                owner: None,
                priority: new_priority,
            },
        );
    }

    /// Register a command with module ownership.
    ///
    /// When the owning module is unloaded, this command will be automatically
    /// deregistered via [`Self::unregister_for_module`].
    pub fn register_for_module(&mut self, handler: Arc<dyn Command>, owner: ModuleId) {
        let id = handler.id();
        let new_priority = handler.priority();

        if self
            .entries
            .get(&id)
            .is_some_and(|existing| new_priority < existing.priority)
        {
            return;
        }

        self.entries.insert(
            id,
            CommandEntry {
                handler,
                owner: Some(owner),
                priority: new_priority,
            },
        );
    }

    /// Remove all commands owned by a module.
    ///
    /// Called when a module is being unloaded to clean up its registrations.
    ///
    /// Returns the number of commands that were removed.
    pub fn unregister_for_module(&mut self, module: &ModuleId) -> usize {
        let before = self.entries.len();
        self.entries
            .retain(|_, entry| entry.owner.as_ref() != Some(module));
        before - self.entries.len()
    }

    /// Get a command by ID.
    #[must_use]
    pub fn get(&self, id: &CommandId) -> Option<&Arc<dyn Command>> {
        self.entries.get(id).map(|entry| &entry.handler)
    }

    /// Check if a command is registered.
    #[must_use]
    pub fn contains(&self, id: &CommandId) -> bool {
        self.entries.contains_key(id)
    }

    // execute_for_client: REMOVED (#753 E6).
    // Command execution routes through DomainDriver.

    /// Get all registered command IDs.
    pub fn ids(&self) -> impl Iterator<Item = &CommandId> {
        self.entries.keys()
    }

    /// Get the number of registered commands.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check if the registry is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Build a [`CommandNameIndex`] from this registry.
    #[must_use]
    pub fn build_name_index(&self) -> reovim_subsys_command::CommandNameIndex {
        let mut index = reovim_subsys_command::CommandNameIndex::new();
        for entry in self.entries.values() {
            let id = entry.handler.id();
            let handler = Arc::clone(&entry.handler);
            for &name in handler.names() {
                index.insert(name.to_string(), id.clone(), Arc::clone(&handler));
            }
        }
        index
    }

    /// Get all command infos for query service.
    #[must_use]
    pub fn all_command_infos(&self) -> Vec<CommandInfo> {
        self.entries
            .values()
            .map(|entry| CommandInfo::from_command(&*entry.handler))
            .collect()
    }
}

impl std::fmt::Debug for CommandRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CommandRegistry")
            .field("count", &self.entries.len())
            .field("commands", &self.entries.keys().collect::<Vec<_>>())
            .finish()
    }
}

// ============================================================================
// CommandQuerySnapshot - Query Service Implementation (#453)
// ============================================================================

/// Snapshot of command metadata for query service.
pub struct CommandQuerySnapshot {
    commands: Vec<CommandInfo>,
}

impl Service for CommandQuerySnapshot {}

impl CommandQuerySnapshot {
    /// Create snapshot from `CommandRegistry`.
    #[must_use]
    pub fn from_registry(registry: &CommandRegistry) -> Self {
        Self {
            commands: registry.all_command_infos(),
        }
    }
}

impl CommandQueryService for CommandQuerySnapshot {
    fn search_by_prefix(&self, prefix: &str) -> Vec<CommandInfo> {
        self.commands
            .iter()
            .filter(|info| info.names.iter().any(|n| n.starts_with(prefix)))
            .cloned()
            .collect()
    }

    fn find_by_name(&self, name: &str) -> Option<CommandInfo> {
        self.commands
            .iter()
            .find(|info| info.names.iter().any(|n| n == name))
            .cloned()
    }

    fn list_user_commands(&self) -> Vec<CommandInfo> {
        self.commands
            .iter()
            .filter(|info| !info.names.is_empty())
            .cloned()
            .collect()
    }

    fn list_all(&self) -> Vec<CommandInfo> {
        self.commands.clone()
    }

    fn count(&self) -> usize {
        self.commands.len()
    }
}

// HandlerBridge and CommandExecutor impl: REMOVED (#753 E6).
// The app layer (bootstrap.rs) creates the bridge to TextDomainDriver.

#[cfg(test)]
#[path = "command_tests.rs"]
mod tests;
