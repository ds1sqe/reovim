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
        CommandContext, CommandHandler, CommandInfo, CommandQueryService, CommandResult,
    },
    reovim_driver_session::{Session as DriverSession, SessionRuntime, api::CommandExecutor},
    reovim_driver_vfs::VfsDriver,
    reovim_kernel::{
        api::v1::{CommandId, KernelContext, ModuleId, Service},
        profile_scope,
    },
};

use crate::app::AppState;

/// Entry in the command registry with optional ownership tracking.
#[derive(Clone)]
struct CommandEntry {
    /// The command handler.
    handler: Arc<dyn CommandHandler>,
    /// The module that owns this command (if any).
    owner: Option<ModuleId>,
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
    /// If a command with the same ID already exists, it is replaced.
    pub fn register(&mut self, handler: Arc<dyn CommandHandler>) {
        let id = handler.id();
        self.entries.insert(
            id,
            CommandEntry {
                handler,
                owner: None,
            },
        );
    }

    /// Register a command handler with module ownership.
    ///
    /// The command's ID is obtained from the handler via its `id()` method.
    /// If a command with the same ID already exists, it is replaced.
    ///
    /// When the owning module is unloaded, this command will be automatically
    /// deregistered via [`Self::unregister_for_module`].
    pub fn register_for_module(&mut self, handler: Arc<dyn CommandHandler>, owner: ModuleId) {
        let id = handler.id();
        self.entries.insert(
            id,
            CommandEntry {
                handler,
                owner: Some(owner),
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
    /// * `client_mode_stack` - Per-client mode stack (source of truth for mode)
    /// * `client_windows` - Per-client window layout (source of truth for cursor)
    /// * `client_extensions` - Per-client module extensions (#477)
    /// * `app` - Application state (contains `KernelContext`)
    /// * `vfs` - VFS driver for file operations
    /// * `args` - Command arguments (count, register, etc.)
    #[must_use]
    #[allow(clippy::too_many_arguments)] // Per-client execution needs all these parameters
    pub fn execute_for_client(
        &self,
        client_id: usize,
        id: &CommandId,
        driver_session: &mut DriverSession,
        client_mode_stack: &mut reovim_kernel::api::v1::ModeStack,
        client_windows: &mut reovim_driver_session::WindowLayout,
        client_extensions: &mut reovim_driver_session::ExtensionMap,
        app: &AppState,
        vfs: &Arc<dyn VfsDriver>,
        args: &CommandContext,
    ) -> Option<(CommandResult, reovim_driver_session::api::StateChanges)> {
        use reovim_driver_session::{ClientId as DriverClientId, api::ChangeTracker};
        profile_scope!("command_execute_for_client", "server::command");

        self.entries.get(id).map(|entry| {
            // Single clone point for context enrichment (Epic #415)
            let mut ctx = args.clone();
            if let Some(buffer_id) = driver_session.active_buffer() {
                ctx.set_buffer_id(buffer_id);
            }
            ctx.set_vfs(Arc::clone(vfs));

            // Create SessionRuntime with per-client state and owner (#471, #477)
            // The owner enables undo_mine()/redo_mine() for per-client undo
            let stub_executor = StubCommandExecutor;
            let driver_client_id = DriverClientId::new(client_id);
            let mut runtime = SessionRuntime::with_owner(
                driver_client_id,
                driver_session,
                client_mode_stack,
                client_windows,
                client_extensions,
                &app.kernel,
                &stub_executor,
            );

            // Execute command
            let result = entry.handler.execute(&mut runtime, &ctx);

            // Take accumulated changes (selection, buffer mods, mode changes, etc.)
            let changes = runtime.take_changes();

            (result, changes)
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

    fn list_ex_commands(&self) -> Vec<CommandInfo> {
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

// === Stub CommandExecutor for internal use ===

/// Stub executor that returns an error for any command execution.
///
/// Used when creating `SessionRuntime` for command execution - commands
/// should not recursively call other commands via `execute_command()`.
struct StubCommandExecutor;

impl CommandExecutor for StubCommandExecutor {
    fn execute(
        &self,
        _cmd: &CommandId,
        _ctx: &CommandContext,
        _kernel: &KernelContext,
    ) -> Option<CommandResult> {
        // Commands cannot recursively execute other commands via SessionRuntime
        Some(CommandResult::Error("recursive command execution not supported".to_string()))
    }
}

// === CommandExecutor implementation for CommandRegistry ===

impl CommandExecutor for CommandRegistry {
    fn execute(
        &self,
        _cmd: &CommandId,
        _ctx: &CommandContext,
        _kernel: &KernelContext,
    ) -> Option<CommandResult> {
        profile_scope!("command_execute_via_executor", "server::command");

        // NOTE: This implementation is complex because commands now need SessionRuntime,
        // but CommandExecutor only provides KernelContext. For now, return an error.
        Some(CommandResult::Error(
            "command execution via CommandExecutor not yet implemented".to_string(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        reovim_driver_command::{ArgSpec, Command},
        reovim_driver_session::ClientId,
        reovim_driver_vfs::MockVfs,
        reovim_kernel::api::v1::ModuleId,
    };

    fn test_vfs() -> Arc<dyn VfsDriver> {
        Arc::new(MockVfs::new())
    }

    // Test command implementation
    struct TestCommand {
        id: CommandId,
    }

    impl TestCommand {
        fn new(name: &'static str) -> Self {
            Self {
                id: CommandId::new(ModuleId::new("test"), name),
            }
        }
    }

    impl Command for TestCommand {
        fn id(&self) -> CommandId {
            self.id.clone()
        }

        fn description(&self) -> &'static str {
            "Test command"
        }

        fn args(&self) -> Vec<ArgSpec> {
            vec![]
        }
    }

    impl CommandHandler for TestCommand {
        fn execute(
            &self,
            _runtime: &mut SessionRuntime<'_>,
            _args: &CommandContext,
        ) -> CommandResult {
            CommandResult::Success
        }
    }

    #[test]
    fn test_command_registry_new() {
        let registry = CommandRegistry::new();
        assert!(registry.is_empty());
        assert_eq!(registry.len(), 0);
    }

    #[test]
    fn test_command_registry_register() {
        let mut registry = CommandRegistry::new();
        let cmd = TestCommand::new("test-cmd");
        let id = cmd.id.clone();

        registry.register(Arc::new(cmd));

        assert!(registry.contains(&id));
        assert_eq!(registry.len(), 1);
    }

    #[test]
    fn test_command_registry_get() {
        let mut registry = CommandRegistry::new();
        let cmd = TestCommand::new("my-cmd");
        let id = cmd.id.clone();

        registry.register(Arc::new(cmd));

        let handler = registry.get(&id);
        assert!(handler.is_some());
        assert_eq!(handler.unwrap().description(), "Test command");
    }

    #[test]
    fn test_command_registry_execute_for_client() {
        let mut registry = CommandRegistry::new();
        let cmd = TestCommand::new("exec-cmd");
        let id = cmd.id.clone();

        registry.register(Arc::new(cmd));

        let kernel = KernelContext::default();
        let mode = reovim_kernel::api::v1::ModeId::new(ModuleId::new("test"), "normal");
        let mut driver_session = DriverSession::new(ClientId::new(0), mode.clone()); // #491
        let app = AppState::new(kernel);
        let vfs = test_vfs();
        let args = CommandContext::new();

        // Per-client state
        let mut client_mode_stack = reovim_kernel::api::v1::ModeStack::new(mode);
        let mut client_windows = reovim_driver_session::WindowLayout::empty();
        let mut client_extensions = reovim_driver_session::ExtensionMap::new();

        let result = registry.execute_for_client(
            1, // test client_id for per-client undo (#471)
            &id,
            &mut driver_session,
            &mut client_mode_stack,
            &mut client_windows,
            &mut client_extensions,
            &app,
            &vfs,
            &args,
        );
        assert!(result.is_some());
        let (cmd_result, _changes) = result.unwrap();
        assert_eq!(cmd_result, CommandResult::Success);
    }

    #[test]
    fn test_command_registry_unregister_for_module() {
        let mut registry = CommandRegistry::new();
        let owner = ModuleId::new("my-module");

        // Register two commands for the module
        registry.register_for_module(Arc::new(TestCommand::new("cmd1")), owner.clone());
        registry.register_for_module(Arc::new(TestCommand::new("cmd2")), owner.clone());

        // Register one command without owner
        registry.register(Arc::new(TestCommand::new("cmd3")));

        assert_eq!(registry.len(), 3);

        // Unregister module's commands
        let removed = registry.unregister_for_module(&owner);

        assert_eq!(removed, 2);
        assert_eq!(registry.len(), 1);
    }
}
