//! Command registry for storing and executing commands.
//!
//! Commands are stored by their [`CommandId`] and executed through
//! the [`CommandHandler`] trait. The registry provides lookup and
//! execution services to the server.
//!
//! # Module Ownership
//!
//! Commands can be registered with optional module ownership via
//! [`CommandRegistry::register_for_module`]. When a module is unloaded, all its
//! registered commands can be removed via [`CommandRegistry::unregister_for_module`].

use std::{collections::HashMap, sync::Arc};

use {
    reovim_driver_command::{
        CommandContext, CommandHandler, CommandInfo, CommandPriority, CommandQueryService,
        CommandResult,
    },
    reovim_driver_session::{
        Session as DriverSession, SessionRuntime,
        api::{CommandExecutor, CommandHandle},
    },
    reovim_driver_vfs::VfsDriver,
    reovim_kernel::{
        api::v1::{CommandId, KernelContext, ModuleId, Service},
        profile_scope,
    },
};

/// Entry in the command registry with optional ownership tracking.
#[derive(Clone)]
struct CommandEntry {
    /// The command handler.
    handler: Arc<dyn CommandHandler>,
    /// The module that owns this command (if any).
    owner: Option<ModuleId>,
    /// Registration priority (#545). Higher priority wins on conflict.
    priority: CommandPriority,
}

/// Registry for command handlers.
///
/// Stores [`CommandHandler`] implementations keyed by [`CommandId`].
/// The server uses this to execute commands when keybindings match.
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

    /// Register a command handler (without module ownership).
    ///
    /// The command's ID is obtained from the handler via its `id()` method.
    /// If a command with the same ID already exists, the higher priority
    /// handler wins. Equal priority uses last-wins semantics (#545).
    pub fn register(&mut self, handler: Arc<dyn CommandHandler>) {
        let id = handler.id();
        let new_priority = handler.priority();

        // Only replace if new handler has >= priority (#545)
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

    /// Register a command handler with module ownership.
    ///
    /// The command's ID is obtained from the handler via its `id()` method.
    /// If a command with the same ID already exists, the higher priority
    /// handler wins. Equal priority uses last-wins semantics (#545).
    ///
    /// When the owning module is unloaded, this command will be automatically
    /// deregistered via [`Self::unregister_for_module`].
    pub fn register_for_module(&mut self, handler: Arc<dyn CommandHandler>, owner: ModuleId) {
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

    /// Get a command handler by ID.
    #[must_use]
    pub fn get(&self, id: &CommandId) -> Option<&Arc<dyn CommandHandler>> {
        self.entries.get(id).map(|entry| &entry.handler)
    }

    /// Check if a command is registered.
    #[must_use]
    pub fn contains(&self, id: &CommandId) -> bool {
        self.entries.contains_key(id)
    }

    /// Execute a command with per-client state (#471, #477).
    ///
    /// This uses [`SessionRuntime::new`] to ensure commands operate
    /// on per-client mode, cursor, and extension state, enabling multi-client isolation.
    ///
    /// # Arguments
    ///
    /// * `id` - The command ID to execute
    /// * `driver_session` - Driver session (for shared state like buffers)
    /// * `client` - Per-client state bundle (mode, windows, extensions, registers, etc.)
    /// * `kernel` - Kernel context (buffers, event bus, options)
    /// * `vfs` - VFS driver for file operations
    /// * `args` - Command arguments (count, register, etc.)
    /// * `shared_extensions` - Optional shared extension map for cross-client state (#543)
    #[must_use]
    #[allow(clippy::too_many_arguments)] // bundled via ClientContext, remaining are distinct concerns
    pub fn execute_for_client(
        &self,
        client_id: usize,
        id: &CommandId,
        driver_session: &mut DriverSession,
        client: reovim_driver_session::ClientContext<'_>,
        kernel: &KernelContext,
        vfs: &Arc<dyn VfsDriver>,
        args: &CommandContext,
        shared_extensions: Option<&mut reovim_driver_session::ExtensionMap>,
    ) -> Option<(
        CommandResult,
        reovim_driver_session::api::StateChanges,
        Vec<reovim_driver_command_types::RuntimeSignal>,
    )> {
        use reovim_driver_session::{ClientId as DriverClientId, api::ChangeTracker};
        profile_scope!("command_execute_for_client", "server::command");

        self.entries.get(id).map(|entry| {
            // Single clone point for context enrichment (Epic #415)
            let mut ctx = args.clone();
            // Per-client active_buffer (#471)
            if let Some(buffer_id) = *client.active_buffer {
                ctx.set_buffer_id(buffer_id);
            }
            ctx.set_vfs(Arc::clone(vfs));

            // Create SessionRuntime with per-client state and real executor (#471, #477, #515, #547)
            // The owner enables undo_mine()/redo_mine() for per-client undo
            // Passing `self` (CommandRegistry) enables re-entrant command execution
            let driver_client_id = DriverClientId::new(client_id);
            let mut runtime =
                SessionRuntime::with_owner(driver_client_id, driver_session, client, kernel, self);

            // Wire up session-wide shared extensions (#543)
            if let Some(ext) = shared_extensions {
                runtime = runtime.with_shared_extensions(ext);
            }

            // Execute command
            let result = entry.handler.execute(&mut runtime, &ctx);

            // Take accumulated changes and signals (#547)
            let changes = runtime.take_changes();
            let signals = runtime.take_signals();

            (result, changes, signals)
        })
    }

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
    ///
    /// Iterates all registered handlers and maps each name alias to
    /// the command's ID and `Command` trait object. The resulting index
    /// is stored in `ServiceRegistry` for vim dispatch (#547).
    #[must_use]
    pub fn build_name_index(&self) -> reovim_driver_command::CommandNameIndex {
        let mut index = reovim_driver_command::CommandNameIndex::new();
        for entry in self.entries.values() {
            let id = entry.handler.id();
            let handler = Arc::clone(&entry.handler);
            let cmd: Arc<dyn reovim_driver_command::Command> = handler;
            for &name in cmd.names() {
                index.insert(name.to_string(), id.clone(), Arc::clone(&cmd));
            }
        }
        index
    }

    /// Get all command infos for query service.
    ///
    /// Used by [`CommandQuerySnapshot`] to capture command metadata.
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
///
/// Captures all command info at bootstrap time for module queries.
/// Commands are static after module loading, so snapshot is sufficient.
pub struct CommandQuerySnapshot {
    commands: Vec<CommandInfo>,
}

impl Service for CommandQuerySnapshot {}

impl CommandQuerySnapshot {
    /// Create snapshot from `CommandRegistry`.
    ///
    /// Captures all command metadata at the time of creation.
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

// === HandlerBridge: CommandHandler -> CommandHandle (#547) ===

/// Bridge from `CommandHandler` (command crate) to `CommandHandle` (session crate).
///
/// Wraps an `Arc<dyn CommandHandler>` so it can be returned from
/// `CommandExecutor::get_handle()`. This breaks the session -> command
/// dependency cycle while enabling re-entrant command execution.
struct HandlerBridge(Arc<dyn CommandHandler>);

impl CommandHandle for HandlerBridge {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, ctx: &CommandContext) -> CommandResult {
        self.0.execute(runtime, ctx)
    }
}

// === CommandExecutor implementation for CommandRegistry ===

impl CommandExecutor for CommandRegistry {
    fn get_handle(&self, id: &CommandId) -> Option<Arc<dyn CommandHandle>> {
        self.entries.get(id).map(|entry| {
            Arc::new(HandlerBridge(Arc::clone(&entry.handler))) as Arc<dyn CommandHandle>
        })
    }
}

#[cfg(test)]
#[path = "command_tests.rs"]
mod tests;
