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
        profile_scope!("command_execute", "runner::command");

        self.entries.get(id).map(|entry| {
            // Clone args and populate buffer ID from AppState
            let mut ctx = args.clone();
            if let Some(buffer_id) = app.active_buffer {
                ctx.set_buffer_id(buffer_id);
            }
            // Note: CommandHandler::execute takes &mut KernelContext,
            // we extract it from AppState
            entry.handler.execute(&mut app.kernel, &ctx)
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

#[cfg(test)]
mod tests {
    use {
        super::*,
        reovim_driver_command::{ArgSpec, Command},
        reovim_kernel::api::v1::{KernelContext, ModuleId},
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
        fn execute(&self, _ctx: &mut KernelContext, _args: &CommandContext) -> CommandResult {
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
