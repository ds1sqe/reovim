//! Command registry for storing and executing commands.
//!
//! Commands are stored by their [`CommandId`] and executed through
//! the [`CommandHandler`] trait. The registry provides lookup and
//! execution services to the event loop.
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

use crate::AppState;

/// Entry in the command registry with optional ownership tracking.
struct CommandEntry {
    /// The command handler.
    handler: Arc<dyn CommandHandler>,
    /// The module that owns this command (if any).
    owner: Option<ModuleId>,
}

/// Registry for command handlers.
///
/// Stores [`CommandHandler`] implementations keyed by [`CommandId`].
/// The event loop uses this to execute commands when keybindings match.
///
/// # Module Ownership
///
/// Commands can be registered with module ownership via [`Self::register_for_module`].
/// This enables automatic cleanup when modules are unloaded.
///
/// # Example
///
/// ```ignore
/// use runner::registry::CommandRegistry;
/// use std::sync::Arc;
///
/// let mut registry = CommandRegistry::new();
/// registry.register(Arc::new(MyCursorDown));
///
/// let cmd_id = MyCursorDown.id();
/// if let Some(result) = registry.execute(&cmd_id, &mut app, &context) {
///     // Handle result
/// }
/// ```
#[derive(Default)]
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

    /// Execute a command by ID.
    ///
    /// Returns `None` if the command isn't registered.
    ///
    /// The active buffer ID from `driver_session` is automatically populated
    /// into the `CommandContext` before execution, allowing commands to
    /// know which buffer they should operate on.
    ///
    /// # Arguments
    ///
    /// * `id` - The command ID to execute
    /// * `driver_session` - Driver session (SSOT for `mode_stack`, `active_buffer`)
    /// * `app` - Application state (contains `KernelContext` + `WindowRegistry`)
    /// * `args` - Command arguments (count, register, etc.)
    ///
    /// # Returns
    ///
    /// `Some(CommandResult)` if the command was found and executed,
    /// `None` if the command wasn't registered.
    #[must_use]
    /// Execute a command by ID.
    ///
    /// # Arguments
    ///
    /// * `id` - Command ID to execute
    /// * `driver_session` - Driver session for state access
    /// * `app` - Application state (kernel, windows, etc.)
    /// * `vfs` - VFS driver for file operations
    /// * `args` - Command context with arguments
    ///
    /// # Context Population (Epic #415)
    ///
    /// All context enrichment happens here in a single clone:
    /// - `buffer_id` from `driver_session`
    /// - `vfs` from the VFS provider registry
    pub fn execute(
        &self,
        id: &CommandId,
        driver_session: &mut DriverSession,
        app: &mut AppState,
        vfs: &Arc<dyn VfsDriver>,
        args: &CommandContext,
    ) -> Option<CommandResult> {
        profile_scope!("command_execute", "runner::command");

        self.entries.get(id).map(|entry| {
            // Single clone point for context enrichment (Epic #415)
            let mut ctx = args.clone();
            if let Some(buffer_id) = driver_session.active_buffer() {
                ctx.set_buffer_id(buffer_id);
            }
            ctx.set_vfs(Arc::clone(vfs));

            // Create SessionRuntime for command execution
            // SessionRuntime uses driver_session.windows (SSOT for window state)
            // No need to copy/sync - driver_session.windows IS the window state
            let stub_executor = StubCommandExecutor;
            let mut runtime = SessionRuntime::new(driver_session, &app.kernel, &stub_executor);

            // Execute command
            entry.handler.execute(&mut runtime, &ctx)
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
///
/// # Usage
///
/// ```ignore
/// // In bootstrap:
/// let snapshot = CommandQuerySnapshot::from_registry(&command_registry);
/// services.register(Arc::new(snapshot));
///
/// // In modules:
/// let query = services.get::<dyn CommandQueryService>()?;
/// let matches = query.search_by_prefix("wri");
/// ```
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
//
// Used when executing commands via SessionRuntime - commands shouldn't
// recursively execute other commands through SessionRuntime.execute_command().

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
        _kernel: &mut KernelContext,
    ) -> Option<CommandResult> {
        // Commands cannot recursively execute other commands via SessionRuntime
        Some(CommandResult::Error("recursive command execution not supported".to_string()))
    }
}

// === CommandExecutor implementation for CommandRegistry ===
//
// This allows SessionRuntime to execute commands without needing
// direct access to AppState. Used by the Session Driver API.
// NOTE: This implementation is currently a placeholder for Phase 3.

impl CommandExecutor for CommandRegistry {
    fn execute(
        &self,
        _cmd: &CommandId,
        _ctx: &CommandContext,
        _kernel: &mut KernelContext,
    ) -> Option<CommandResult> {
        profile_scope!("command_execute_via_executor", "runner::command");

        // NOTE: This implementation is complex because commands now need SessionRuntime,
        // but CommandExecutor only provides KernelContext. For now, return an error.
        // Commands that need to call other commands should use the event-based approach.
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
        reovim_driver_session::{ClientId, SessionRuntime},
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
    fn test_command_registry_execute() {
        let mut registry = CommandRegistry::new();
        let cmd = TestCommand::new("exec-cmd");
        let id = cmd.id.clone();

        registry.register(Arc::new(cmd));

        let kernel = KernelContext::default();
        let mode = reovim_kernel::api::v1::ModeId::new(ModuleId::new("test"), "normal");
        let mut driver_session = DriverSession::new(ClientId::new(0), mode);
        let mut app = AppState::new(kernel);
        let vfs = test_vfs();
        let args = CommandContext::new();

        let result = registry.execute(&id, &mut driver_session, &mut app, &vfs, &args);
        assert_eq!(result, Some(CommandResult::Success));
    }

    #[test]
    fn test_command_registry_execute_not_found() {
        let registry = CommandRegistry::new();
        let unknown_id = CommandId::new(ModuleId::new("unknown"), "cmd");

        let kernel = KernelContext::default();
        let mode = reovim_kernel::api::v1::ModeId::new(ModuleId::new("test"), "normal");
        let mut driver_session = DriverSession::new(ClientId::new(0), mode);
        let mut app = AppState::new(kernel);
        let vfs = test_vfs();
        let args = CommandContext::new();

        let result = registry.execute(&unknown_id, &mut driver_session, &mut app, &vfs, &args);
        assert!(result.is_none());
    }

    #[test]
    fn test_command_registry_ids() {
        let mut registry = CommandRegistry::new();
        registry.register(Arc::new(TestCommand::new("cmd1")));
        registry.register(Arc::new(TestCommand::new("cmd2")));

        assert_eq!(registry.ids().count(), 2);
    }

    #[test]
    fn test_command_registry_register_for_module() {
        let mut registry = CommandRegistry::new();
        let cmd = TestCommand::new("owned-cmd");
        let id = cmd.id.clone();
        let owner = ModuleId::new("my-module");

        registry.register_for_module(Arc::new(cmd), owner);

        assert!(registry.contains(&id));
        assert_eq!(registry.len(), 1);
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
        assert!(!registry.contains(&CommandId::new(ModuleId::new("test"), "cmd1")));
        assert!(!registry.contains(&CommandId::new(ModuleId::new("test"), "cmd2")));
        assert!(registry.contains(&CommandId::new(ModuleId::new("test"), "cmd3")));
    }

    #[test]
    fn test_command_registry_unregister_for_module_empty() {
        let mut registry = CommandRegistry::new();
        let owner = ModuleId::new("my-module");

        // Register commands without owner
        registry.register(Arc::new(TestCommand::new("cmd1")));

        // Try to unregister for a module that has no commands
        let removed = registry.unregister_for_module(&owner);

        assert_eq!(removed, 0);
        assert_eq!(registry.len(), 1);
    }

    #[test]
    fn test_command_registry_multiple_modules() {
        let mut registry = CommandRegistry::new();
        let module_x = ModuleId::new("module-a");
        let module_y = ModuleId::new("module-b");

        registry.register_for_module(Arc::new(TestCommand::new("a-cmd")), module_x.clone());
        registry.register_for_module(Arc::new(TestCommand::new("b-cmd")), module_y);

        assert_eq!(registry.len(), 2);

        // Unload module A
        registry.unregister_for_module(&module_x);

        assert_eq!(registry.len(), 1);
        assert!(!registry.contains(&CommandId::new(ModuleId::new("test"), "a-cmd")));
        assert!(registry.contains(&CommandId::new(ModuleId::new("test"), "b-cmd")));
    }

    // ========================================================================
    // CommandQuerySnapshot Tests (#453)
    // ========================================================================

    // Test command with ex-names
    struct ExCommand {
        id: CommandId,
        names: &'static [&'static str],
    }

    impl ExCommand {
        fn new(name: &'static str, ex_names: &'static [&'static str]) -> Self {
            Self {
                id: CommandId::new(ModuleId::new("test"), name),
                names: ex_names,
            }
        }
    }

    impl Command for ExCommand {
        fn id(&self) -> CommandId {
            self.id.clone()
        }

        fn description(&self) -> &'static str {
            "Ex command"
        }

        fn args(&self) -> Vec<ArgSpec> {
            vec![]
        }

        fn names(&self) -> &[&'static str] {
            self.names
        }
    }

    impl CommandHandler for ExCommand {
        fn execute(
            &self,
            _runtime: &mut SessionRuntime<'_>,
            _args: &CommandContext,
        ) -> CommandResult {
            CommandResult::Success
        }
    }

    #[test]
    fn test_command_query_snapshot_from_registry() {
        let mut registry = CommandRegistry::new();
        registry.register(Arc::new(ExCommand::new("write-cmd", &["write", "w"])));
        registry.register(Arc::new(ExCommand::new("quit-cmd", &["quit", "q"])));

        let snapshot = CommandQuerySnapshot::from_registry(&registry);
        assert_eq!(snapshot.count(), 2);
    }

    #[test]
    fn test_command_query_snapshot_search_by_prefix() {
        let mut registry = CommandRegistry::new();
        registry.register(Arc::new(ExCommand::new("write-cmd", &["write", "w"])));
        registry.register(Arc::new(ExCommand::new("wq-cmd", &["wq", "writequit"])));
        registry.register(Arc::new(ExCommand::new("quit-cmd", &["quit", "q"])));

        let snapshot = CommandQuerySnapshot::from_registry(&registry);

        // Search for "w" prefix
        let matches = snapshot.search_by_prefix("w");
        assert_eq!(matches.len(), 2); // write and wq both start with "w"

        // Search for "wri" prefix
        let matches = snapshot.search_by_prefix("wri");
        assert_eq!(matches.len(), 2); // "write" and "writequit"

        // Search for "q" prefix
        let matches = snapshot.search_by_prefix("q");
        assert_eq!(matches.len(), 1); // only "quit"
    }

    #[test]
    fn test_command_query_snapshot_search_by_prefix_empty() {
        let mut registry = CommandRegistry::new();
        registry.register(Arc::new(ExCommand::new("write-cmd", &["write", "w"])));

        let snapshot = CommandQuerySnapshot::from_registry(&registry);

        // Empty prefix matches commands with any ex-name
        let matches = snapshot.search_by_prefix("");
        assert_eq!(matches.len(), 1);
    }

    #[test]
    fn test_command_query_snapshot_find_by_name() {
        let mut registry = CommandRegistry::new();
        registry.register(Arc::new(ExCommand::new("write-cmd", &["write", "w"])));
        registry.register(Arc::new(ExCommand::new("quit-cmd", &["quit", "q"])));

        let snapshot = CommandQuerySnapshot::from_registry(&registry);

        // Find by full name
        let info = snapshot.find_by_name("write");
        assert!(info.is_some());
        assert_eq!(info.unwrap().id.name(), "write-cmd");

        // Find by alias
        let info = snapshot.find_by_name("w");
        assert!(info.is_some());
        assert_eq!(info.unwrap().id.name(), "write-cmd");

        // Not found
        let info = snapshot.find_by_name("nonexistent");
        assert!(info.is_none());
    }

    #[test]
    fn test_command_query_snapshot_list_ex_commands() {
        let mut registry = CommandRegistry::new();
        registry.register(Arc::new(ExCommand::new("write-cmd", &["write", "w"])));
        registry.register(Arc::new(ExCommand::new("internal-cmd", &[]))); // No ex-names

        let snapshot = CommandQuerySnapshot::from_registry(&registry);

        let ex_cmds = snapshot.list_ex_commands();
        assert_eq!(ex_cmds.len(), 1);
        assert_eq!(ex_cmds[0].id.name(), "write-cmd");
    }

    #[test]
    fn test_command_query_snapshot_list_all() {
        let mut registry = CommandRegistry::new();
        registry.register(Arc::new(ExCommand::new("write-cmd", &["write", "w"])));
        registry.register(Arc::new(ExCommand::new("internal-cmd", &[])));

        let snapshot = CommandQuerySnapshot::from_registry(&registry);

        let all_cmds = snapshot.list_all();
        assert_eq!(all_cmds.len(), 2);
    }
}
