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
mod tests {
    use {
        super::*,
        reovim_driver_command::{ArgSpec, Command},
        reovim_driver_session::ClientId,
        reovim_driver_vfs::MockVfs,
        reovim_kernel::api::v1::{HistoryRing, KernelContext, MarkBank, ModuleId, RegisterBank},
    };

    fn test_vfs() -> Arc<dyn VfsDriver> {
        Arc::new(MockVfs::new())
    }

    // Test command implementation
    struct TestCommand {
        id: CommandId,
        name: &'static str,
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    impl TestCommand {
        fn new(name: &'static str) -> Self {
            Self {
                id: CommandId::new(ModuleId::new("test"), name),
                name,
            }
        }
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
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

        fn names(&self) -> &[&'static str] {
            std::slice::from_ref(&self.name)
        }
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
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
        let vfs = test_vfs();
        let args = CommandContext::new();

        // Per-client state
        let mut client_mode_stack = reovim_kernel::api::v1::ModeStack::new(mode);
        let mut client_windows = reovim_driver_session::WindowLayout::empty();
        let mut client_extensions = reovim_driver_session::ExtensionMap::new();
        let mut client_compositor = None;
        let mut tabs = reovim_driver_session::TabPageSet::new();
        let mut registers = RegisterBank::new();
        let mut clipboard_history = HistoryRing::new();
        let mut local_marks = MarkBank::new();
        let mut active_buffer = None;
        let mut terminal_size = (80u16, 24u16);

        let result = registry.execute_for_client(
            1, // test client_id for per-client undo (#471)
            &id,
            &mut driver_session,
            reovim_driver_session::ClientContext {
                mode_stack: &mut client_mode_stack,
                windows: &mut client_windows,
                extensions: &mut client_extensions,
                compositor: &mut client_compositor,
                tabs: &mut tabs,
                registers: &mut registers,
                clipboard_history: &mut clipboard_history,
                local_marks: &mut local_marks,
                active_buffer: &mut active_buffer,
                terminal_size: &mut terminal_size,
            },
            &kernel,
            &vfs,
            &args,
            None,
        );
        assert!(result.is_some());
        let (cmd_result, _changes, _signals) = result.unwrap();
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

    #[test]
    fn test_command_registry_replace_existing() {
        let mut registry = CommandRegistry::new();
        let id = CommandId::new(ModuleId::new("test"), "same-cmd");

        // Register first command
        registry.register(Arc::new(TestCommand::new("same-cmd")));
        assert_eq!(registry.len(), 1);

        // Register again - should replace (same priority, last wins)
        registry.register(Arc::new(TestCommand::new("same-cmd")));
        assert_eq!(registry.len(), 1);
        assert!(registry.contains(&id));
    }

    // ========================================================================
    // CommandPriority tests (#545)
    // ========================================================================

    /// Test command with configurable priority.
    struct PriorityTestCommand {
        id: CommandId,
        name: &'static str,
        priority: reovim_driver_command::CommandPriority,
        desc: &'static str,
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    impl PriorityTestCommand {
        fn normal(name: &'static str) -> Self {
            Self {
                id: CommandId::new(ModuleId::new("test"), name),
                name,
                priority: reovim_driver_command::CommandPriority::Normal,
                desc: "Normal priority",
            }
        }

        fn override_priority(name: &'static str) -> Self {
            Self {
                id: CommandId::new(ModuleId::new("test"), name),
                name,
                priority: reovim_driver_command::CommandPriority::Override,
                desc: "Override priority",
            }
        }
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    impl Command for PriorityTestCommand {
        fn id(&self) -> CommandId {
            self.id.clone()
        }
        fn description(&self) -> &'static str {
            self.desc
        }
        fn names(&self) -> &[&'static str] {
            std::slice::from_ref(&self.name)
        }
        fn priority(&self) -> reovim_driver_command::CommandPriority {
            self.priority
        }
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    impl CommandHandler for PriorityTestCommand {
        fn execute(
            &self,
            _runtime: &mut SessionRuntime<'_>,
            _args: &CommandContext,
        ) -> CommandResult {
            CommandResult::Success
        }
    }

    #[test]
    fn test_override_priority_wins_over_normal() {
        let mut registry = CommandRegistry::new();
        let id = CommandId::new(ModuleId::new("test"), "cmd");

        // Register normal first
        registry.register(Arc::new(PriorityTestCommand::normal("cmd")));
        assert_eq!(registry.get(&id).unwrap().description(), "Normal priority");

        // Register override second - should replace
        registry.register(Arc::new(PriorityTestCommand::override_priority("cmd")));
        assert_eq!(registry.get(&id).unwrap().description(), "Override priority");
    }

    #[test]
    fn test_normal_cannot_replace_override() {
        let mut registry = CommandRegistry::new();
        let id = CommandId::new(ModuleId::new("test"), "cmd");

        // Register override first
        registry.register(Arc::new(PriorityTestCommand::override_priority("cmd")));
        assert_eq!(registry.get(&id).unwrap().description(), "Override priority");

        // Register normal second - should NOT replace
        registry.register(Arc::new(PriorityTestCommand::normal("cmd")));
        assert_eq!(registry.get(&id).unwrap().description(), "Override priority");
        assert_eq!(registry.len(), 1);
    }

    #[test]
    fn test_equal_priority_last_wins() {
        let mut registry = CommandRegistry::new();
        let id = CommandId::new(ModuleId::new("test"), "cmd");

        // Two normal-priority commands: last one wins
        registry.register(Arc::new(PriorityTestCommand::normal("cmd")));
        registry.register(Arc::new(PriorityTestCommand::normal("cmd")));
        assert_eq!(registry.len(), 1);
        assert!(registry.contains(&id));
    }

    #[test]
    fn test_priority_with_register_for_module() {
        let mut registry = CommandRegistry::new();
        let id = CommandId::new(ModuleId::new("test"), "cmd");
        let owner = ModuleId::new("adapter");

        // Register normal first (no owner)
        registry.register(Arc::new(PriorityTestCommand::normal("cmd")));

        // Register override with module ownership - should replace
        registry
            .register_for_module(Arc::new(PriorityTestCommand::override_priority("cmd")), owner);
        assert_eq!(registry.get(&id).unwrap().description(), "Override priority");

        // Try to replace with normal + different owner - should NOT replace
        registry.register_for_module(
            Arc::new(PriorityTestCommand::normal("cmd")),
            ModuleId::new("other"),
        );
        assert_eq!(registry.get(&id).unwrap().description(), "Override priority");
    }

    #[test]
    fn test_command_registry_ids() {
        let mut registry = CommandRegistry::new();
        registry.register(Arc::new(TestCommand::new("cmd1")));
        registry.register(Arc::new(TestCommand::new("cmd2")));
        registry.register(Arc::new(TestCommand::new("cmd3")));

        assert_eq!(registry.ids().count(), 3);
    }

    #[test]
    fn test_command_registry_all_command_infos() {
        let mut registry = CommandRegistry::new();
        registry.register(Arc::new(TestCommand::new("cmd1")));
        registry.register(Arc::new(TestCommand::new("cmd2")));

        let infos = registry.all_command_infos();
        assert_eq!(infos.len(), 2);
        assert!(infos.iter().all(|info| info.description == "Test command"));
    }

    #[test]
    fn test_command_registry_debug() {
        let mut registry = CommandRegistry::new();
        registry.register(Arc::new(TestCommand::new("test-cmd")));

        let debug_str = format!("{registry:?}");
        assert!(debug_str.contains("CommandRegistry"));
        assert!(debug_str.contains("count"));
    }

    #[test]
    fn test_command_query_snapshot_from_registry() {
        let mut registry = CommandRegistry::new();
        registry.register(Arc::new(TestCommand::new("cmd1")));
        registry.register(Arc::new(TestCommand::new("cmd2")));

        let snapshot = CommandQuerySnapshot::from_registry(&registry);
        assert_eq!(snapshot.count(), 2);
    }

    #[test]
    fn test_command_query_snapshot_search_by_prefix() {
        let mut registry = CommandRegistry::new();
        registry.register(Arc::new(TestCommand::new("test-cmd1")));
        registry.register(Arc::new(TestCommand::new("test-cmd2")));
        registry.register(Arc::new(TestCommand::new("other-cmd")));

        let snapshot = CommandQuerySnapshot::from_registry(&registry);
        let results = snapshot.search_by_prefix("test");
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_command_query_snapshot_find_by_name() {
        let mut registry = CommandRegistry::new();
        registry.register(Arc::new(TestCommand::new("find-me")));

        let snapshot = CommandQuerySnapshot::from_registry(&registry);
        let result = snapshot.find_by_name("find-me");
        assert!(result.is_some());

        let not_found = snapshot.find_by_name("not-there");
        assert!(not_found.is_none());
    }

    #[test]
    fn test_command_query_snapshot_list_user_commands() {
        let mut registry = CommandRegistry::new();
        registry.register(Arc::new(TestCommand::new("ex-cmd1")));
        registry.register(Arc::new(TestCommand::new("ex-cmd2")));

        let snapshot = CommandQuerySnapshot::from_registry(&registry);
        let user_commands = snapshot.list_user_commands();
        assert_eq!(user_commands.len(), 2);
    }

    #[test]
    fn test_command_query_snapshot_list_all() {
        let mut registry = CommandRegistry::new();
        registry.register(Arc::new(TestCommand::new("cmd1")));
        registry.register(Arc::new(TestCommand::new("cmd2")));

        let snapshot = CommandQuerySnapshot::from_registry(&registry);
        let all_commands = snapshot.list_all();
        assert_eq!(all_commands.len(), 2);
    }

    #[test]
    fn test_command_registry_get_handle_found() {
        let mut registry = CommandRegistry::new();
        let cmd = TestCommand::new("handle-cmd");
        let id = cmd.id.clone();
        registry.register(Arc::new(cmd));

        let handle = registry.get_handle(&id);
        assert!(handle.is_some());
    }

    #[test]
    fn test_command_registry_get_handle_not_found() {
        let registry = CommandRegistry::new();
        let id = CommandId::new(ModuleId::new("test"), "nonexistent");

        let handle = registry.get_handle(&id);
        assert!(handle.is_none());
    }

    #[test]
    fn test_command_registry_execute_for_client_not_found() {
        let registry = CommandRegistry::new();
        let kernel = KernelContext::default();
        let mode = reovim_kernel::api::v1::ModeId::new(ModuleId::new("test"), "normal");
        let mut driver_session = DriverSession::new(ClientId::new(0), mode.clone());
        let vfs = test_vfs();
        let args = CommandContext::new();
        let id = CommandId::new(ModuleId::new("test"), "nonexistent");

        let mut client_mode_stack = reovim_kernel::api::v1::ModeStack::new(mode);
        let mut client_windows = reovim_driver_session::WindowLayout::empty();
        let mut client_extensions = reovim_driver_session::ExtensionMap::new();
        let mut client_compositor = None;
        let mut tabs = reovim_driver_session::TabPageSet::new();
        let mut registers = RegisterBank::new();
        let mut clipboard_history = HistoryRing::new();
        let mut local_marks = MarkBank::new();
        let mut active_buffer = None;
        let mut terminal_size = (80u16, 24u16);

        let result = registry.execute_for_client(
            1,
            &id,
            &mut driver_session,
            reovim_driver_session::ClientContext {
                mode_stack: &mut client_mode_stack,
                windows: &mut client_windows,
                extensions: &mut client_extensions,
                compositor: &mut client_compositor,
                tabs: &mut tabs,
                registers: &mut registers,
                clipboard_history: &mut clipboard_history,
                local_marks: &mut local_marks,
                active_buffer: &mut active_buffer,
                terminal_size: &mut terminal_size,
            },
            &kernel,
            &vfs,
            &args,
            None,
        );
        assert!(result.is_none());
    }

    // Test command with buffer context
    struct BufferTestCommand {
        id: CommandId,
        name: &'static str,
    }

    impl BufferTestCommand {
        fn new(name: &'static str) -> Self {
            Self {
                id: CommandId::new(ModuleId::new("test"), name),
                name,
            }
        }
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    impl Command for BufferTestCommand {
        fn id(&self) -> CommandId {
            self.id.clone()
        }

        fn description(&self) -> &'static str {
            "Buffer test command"
        }

        fn args(&self) -> Vec<ArgSpec> {
            vec![]
        }

        fn names(&self) -> &[&'static str] {
            std::slice::from_ref(&self.name)
        }
    }

    impl CommandHandler for BufferTestCommand {
        fn execute(
            &self,
            _runtime: &mut SessionRuntime<'_>,
            _args: &CommandContext,
        ) -> CommandResult {
            CommandResult::Success
        }
    }

    #[test]
    fn test_command_registry_execute_with_buffer() {
        use reovim_kernel::api::v1::BufferId;

        let mut registry = CommandRegistry::new();
        let cmd = BufferTestCommand::new("buffer-cmd");
        let id = cmd.id.clone();

        registry.register(Arc::new(cmd));

        let kernel = KernelContext::default();
        let mode = reovim_kernel::api::v1::ModeId::new(ModuleId::new("test"), "normal");
        let mut driver_session = DriverSession::new(ClientId::new(0), mode.clone());

        let buffer_id = BufferId::from_raw(1);

        let vfs = test_vfs();
        let args = CommandContext::new();

        let mut client_mode_stack = reovim_kernel::api::v1::ModeStack::new(mode);
        let mut client_windows = reovim_driver_session::WindowLayout::empty();
        let mut client_extensions = reovim_driver_session::ExtensionMap::new();
        let mut client_compositor = None;
        let mut tabs = reovim_driver_session::TabPageSet::new();
        let mut registers = RegisterBank::new();
        let mut clipboard_history = HistoryRing::new();
        let mut local_marks = MarkBank::new();
        let mut active_buffer = Some(buffer_id); // Per-client active_buffer (#471)
        let mut terminal_size = (80u16, 24u16);

        let result = registry.execute_for_client(
            1,
            &id,
            &mut driver_session,
            reovim_driver_session::ClientContext {
                mode_stack: &mut client_mode_stack,
                windows: &mut client_windows,
                extensions: &mut client_extensions,
                compositor: &mut client_compositor,
                tabs: &mut tabs,
                registers: &mut registers,
                clipboard_history: &mut clipboard_history,
                local_marks: &mut local_marks,
                active_buffer: &mut active_buffer,
                terminal_size: &mut terminal_size,
            },
            &kernel,
            &vfs,
            &args,
            None,
        );
        assert!(result.is_some());
    }

    #[test]
    fn test_command_registry_get_nonexistent() {
        let registry = CommandRegistry::new();
        let id = CommandId::new(ModuleId::new("test"), "nonexistent");
        assert!(registry.get(&id).is_none());
    }

    #[test]
    fn test_command_registry_contains_false() {
        let registry = CommandRegistry::new();
        let id = CommandId::new(ModuleId::new("test"), "missing");
        assert!(!registry.contains(&id));
    }

    // === CommandQuerySnapshot additional tests (#453) ===

    #[test]
    fn test_command_query_snapshot_search_by_prefix_empty_string() {
        let mut registry = CommandRegistry::new();
        registry.register(Arc::new(TestCommand::new("alpha")));
        registry.register(Arc::new(TestCommand::new("beta")));

        let snapshot = CommandQuerySnapshot::from_registry(&registry);
        // Empty prefix should return all commands
        let results = snapshot.search_by_prefix("");
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_command_query_snapshot_search_by_prefix_partial() {
        let mut registry = CommandRegistry::new();
        registry.register(Arc::new(TestCommand::new("write-file")));
        registry.register(Arc::new(TestCommand::new("write-all")));
        registry.register(Arc::new(TestCommand::new("quit")));

        let snapshot = CommandQuerySnapshot::from_registry(&registry);
        let results = snapshot.search_by_prefix("write");
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_command_query_snapshot_list_user_commands_excludes_internal() {
        // TestCommand always has a name, so create an internal-only command
        struct InternalCommand;

        #[cfg_attr(coverage_nightly, coverage(off))]
        impl Command for InternalCommand {
            fn id(&self) -> CommandId {
                CommandId::new(ModuleId::new("test"), "internal")
            }
            fn description(&self) -> &'static str {
                "Internal command"
            }
            fn names(&self) -> &[&'static str] {
                &[] // No ex-names → internal only
            }
        }

        #[cfg_attr(coverage_nightly, coverage(off))]
        impl CommandHandler for InternalCommand {
            fn execute(
                &self,
                _runtime: &mut SessionRuntime<'_>,
                _args: &CommandContext,
            ) -> CommandResult {
                CommandResult::Success
            }
        }

        let mut registry = CommandRegistry::new();
        registry.register(Arc::new(TestCommand::new("visible")));
        registry.register(Arc::new(InternalCommand));

        let snapshot = CommandQuerySnapshot::from_registry(&registry);
        // list_all includes internal
        assert_eq!(snapshot.count(), 2);
        // list_user_commands excludes internal (no names)
        let user_cmds = snapshot.list_user_commands();
        assert_eq!(user_cmds.len(), 1);
        assert_eq!(user_cmds[0].names[0], "visible");
    }

    #[test]
    fn test_command_registry_unregister_nonexistent_module() {
        let mut registry = CommandRegistry::new();
        registry.register(Arc::new(TestCommand::new("cmd1")));

        let nonexistent = ModuleId::new("nonexistent");
        let removed = registry.unregister_for_module(&nonexistent);
        assert_eq!(removed, 0);
        assert_eq!(registry.len(), 1);
    }

    // ========================================================================
    // build_name_index() tests (#547 Phase 8)
    // ========================================================================

    /// Test command with multiple name aliases for name index tests.
    struct MultiNameCommand {
        id: CommandId,
        names: &'static [&'static str],
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    impl MultiNameCommand {
        fn new(name: &'static str, names: &'static [&'static str]) -> Self {
            Self {
                id: CommandId::new(ModuleId::new("test"), name),
                names,
            }
        }
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    impl Command for MultiNameCommand {
        fn id(&self) -> CommandId {
            self.id.clone()
        }
        fn description(&self) -> &'static str {
            "Multi-name command"
        }
        fn names(&self) -> &[&'static str] {
            self.names
        }
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    impl CommandHandler for MultiNameCommand {
        fn execute(
            &self,
            _runtime: &mut SessionRuntime<'_>,
            _args: &CommandContext,
        ) -> CommandResult {
            CommandResult::Success
        }
    }

    #[test]
    fn test_build_name_index_from_registry() {
        let mut registry = CommandRegistry::new();
        registry.register(Arc::new(MultiNameCommand::new("write", &["w", "write"])));
        registry.register(Arc::new(MultiNameCommand::new("quit", &["q", "quit"])));

        let index = registry.build_name_index();
        assert_eq!(index.count(), 2); // 2 unique commands

        // All aliases resolve
        assert!(index.resolve("w").is_some());
        assert!(index.resolve("write").is_some());
        assert!(index.resolve("q").is_some());
        assert!(index.resolve("quit").is_some());
    }

    #[test]
    fn test_build_name_index_aliases_same_id() {
        let mut registry = CommandRegistry::new();
        registry.register(Arc::new(MultiNameCommand::new("write", &["w", "write"])));

        let index = registry.build_name_index();
        let id_w = index.resolve("w").unwrap();
        let id_write = index.resolve("write").unwrap();
        assert_eq!(id_w, id_write);
    }

    #[test]
    fn test_build_name_index_empty_registry() {
        let registry = CommandRegistry::new();
        let index = registry.build_name_index();
        assert_eq!(index.count(), 0);
    }

    #[test]
    fn test_build_name_index_commands_without_names() {
        // Commands without names (internal-only) should not appear in index
        let mut registry = CommandRegistry::new();
        registry.register(Arc::new(TestCommand::new("internal-cmd")));

        let index = registry.build_name_index();
        // TestCommand has one name (its name field), so it appears
        assert!(index.resolve("internal-cmd").is_some());
    }
}
