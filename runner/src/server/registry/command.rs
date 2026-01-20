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
    reovim_driver_command::{CommandContext, CommandHandler, CommandResult},
    reovim_kernel::{
        api::v1::{CommandId, ModuleId},
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
    /// The active buffer ID from `AppState` is automatically populated
    /// into the `CommandContext` before execution, allowing commands to
    /// know which buffer they should operate on.
    ///
    /// # Arguments
    ///
    /// * `id` - The command ID to execute
    /// * `app` - Application state (contains `KernelContext` + runtime state)
    /// * `args` - Command arguments (count, register, etc.)
    ///
    /// # Returns
    ///
    /// `Some(CommandResult)` if the command was found and executed,
    /// `None` if the command wasn't registered.
    #[must_use]
    pub fn execute(
        &self,
        id: &CommandId,
        app: &mut AppState,
        args: &CommandContext,
    ) -> Option<CommandResult> {
        use reovim_driver_session::{ClientId, Session, SessionRuntime, Window};

        profile_scope!("command_execute", "runner::command");

        self.entries.get(id).map(|entry| {
            // Clone args and populate buffer ID from AppState
            let mut ctx = args.clone();
            if let Some(buffer_id) = app.active_buffer {
                ctx.set_buffer_id(buffer_id);
            }

            // Create a temporary Session from AppState fields for SessionRuntime
            // This bridges the AppState/Session gap during migration
            let mut session = Session::new(ClientId::new(1), app.mode_stack.home().clone());

            // Copy mode stack (excluding home mode which is already set)
            for mode in app.mode_stack.as_slice().iter().skip(1) {
                session.mode_stack.push(mode.clone());
            }

            // Copy windows from WindowRegistry to Session's WindowLayout
            for win_id in app.windows.windows() {
                if let Some(state) = app.windows.get(win_id) {
                    let mut window = Window::new();
                    window.buffer_id = state.buffer_id;
                    window.cursor.line = state.cursor.line;
                    window.cursor.column = state.cursor.column;
                    session.windows.add(window);
                }
            }

            // Create SessionRuntime for command execution
            // NOTE: We pass a stub executor here because commands shouldn't
            // recursively execute other commands via SessionRuntime::execute_command
            let stub_executor = StubCommandExecutor;
            let mut runtime = SessionRuntime::new(&mut session, &app.kernel, &stub_executor);

            // Execute command with new signature
            let result = entry.handler.execute(&mut runtime, &ctx);

            // Sync changes back to AppState
            // Mode stack changes - sync entire stack from Session to AppState
            // First, pop all non-home modes from AppState
            while app.mode_stack.depth() > 1 {
                app.mode_stack.pop();
            }
            // Sync the current mode (handles set_mode changes to home position)
            if let Some(session_current) = session.mode_stack.as_slice().first() {
                app.mode_stack.set(session_current.clone());
            }
            // Push any additional modes from Session
            for mode in session.mode_stack.as_slice().iter().skip(1) {
                app.mode_stack.push(mode.clone());
            }

            // Window cursor changes - sync back to WindowRegistry
            // (buffer content changes go through kernel directly)
            // Collect window IDs first to avoid borrow conflict
            let window_ids: Vec<_> = app.windows.windows().collect();
            for (idx, window) in session.windows.windows.iter().enumerate() {
                if let Some(&win_id) = window_ids.get(idx)
                    && let Some(state) = app.windows.get_mut(win_id)
                {
                    state.cursor.line = window.cursor.line;
                    state.cursor.column = window.cursor.column;
                }
            }

            result
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
}

impl std::fmt::Debug for CommandRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CommandRegistry")
            .field("count", &self.entries.len())
            .field("commands", &self.entries.keys().collect::<Vec<_>>())
            .finish()
    }
}

// === Stub CommandExecutor for internal use ===
//
// Used when executing commands via SessionRuntime - commands shouldn't
// recursively execute other commands through SessionRuntime.execute_command().

use {reovim_driver_session::CommandExecutor, reovim_kernel::api::v1::KernelContext};

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
        reovim_driver_session::SessionRuntime,
        reovim_kernel::api::v1::ModuleId,
    };

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
        let mut app = AppState::new(kernel, mode);
        let args = CommandContext::new();

        let result = registry.execute(&id, &mut app, &args);
        assert_eq!(result, Some(CommandResult::Success));
    }

    #[test]
    fn test_command_registry_execute_not_found() {
        let registry = CommandRegistry::new();
        let unknown_id = CommandId::new(ModuleId::new("unknown"), "cmd");

        let kernel = KernelContext::default();
        let mode = reovim_kernel::api::v1::ModeId::new(ModuleId::new("test"), "normal");
        let mut app = AppState::new(kernel, mode);
        let args = CommandContext::new();

        let result = registry.execute(&unknown_id, &mut app, &args);
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
}
