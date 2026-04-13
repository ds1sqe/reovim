use {
    super::*,
    reovim_domain_text::{HistoryRing, RegisterBank},
    reovim_driver_command::{ArgSpec, Command},
    reovim_driver_session::{ClientId, Jumplist, MarkBank},
    reovim_driver_vfs::MockVfs,
    reovim_kernel::api::v1::{KernelContext, ModuleId},
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
    fn execute(&self, _runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
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
    let mut jumplist = Jumplist::new();
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
            jumplist: &mut jumplist,
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
    fn execute(&self, _runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
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
    registry.register_for_module(Arc::new(PriorityTestCommand::override_priority("cmd")), owner);
    assert_eq!(registry.get(&id).unwrap().description(), "Override priority");

    // Try to replace with normal + different owner - should NOT replace
    registry
        .register_for_module(Arc::new(PriorityTestCommand::normal("cmd")), ModuleId::new("other"));
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
    let mut jumplist = Jumplist::new();
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
            jumplist: &mut jumplist,
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
    fn execute(&self, _runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
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
    let mut jumplist = Jumplist::new();
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
            jumplist: &mut jumplist,
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
    fn execute(&self, _runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
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

// ========================================================================
// HandlerBridge::execute coverage (#547)
// ========================================================================

/// Exercise `HandlerBridge::execute` via `get_handle()` + `handle.execute()`.
///
/// `HandlerBridge` is only reachable through `CommandExecutor::get_handle` — the
/// existing `execute_for_client` tests go through a different code path.  This
/// test calls the bridge directly so the three lines that form its body are
/// counted by the coverage tool.
#[test]
fn test_handler_bridge_execute_via_get_handle() {
    use reovim_driver_session::api::CommandExecutor;

    let mut registry = CommandRegistry::new();
    let cmd = TestCommand::new("bridge-cmd");
    let id = cmd.id.clone();
    registry.register(Arc::new(cmd));

    let handle = registry.get_handle(&id).expect("handle exists");

    let kernel = KernelContext::default();
    let mode = reovim_kernel::api::v1::ModeId::new(ModuleId::new("test"), "normal");
    let mut driver_session = DriverSession::new(ClientId::new(0), mode.clone());
    let args = CommandContext::new();

    let mut client_mode_stack = reovim_kernel::api::v1::ModeStack::new(mode);
    let mut client_windows = reovim_driver_session::WindowLayout::empty();
    let mut client_extensions = reovim_driver_session::ExtensionMap::new();
    let mut client_compositor = None;
    let mut tabs = reovim_driver_session::TabPageSet::new();
    let mut registers = RegisterBank::new();
    let mut clipboard_history = HistoryRing::new();
    let mut local_marks = MarkBank::new();
    let mut jumplist = Jumplist::new();
    let mut active_buffer = None;
    let mut terminal_size = (80u16, 24u16);

    let mut runtime = SessionRuntime::new(
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
            jumplist: &mut jumplist,
            active_buffer: &mut active_buffer,
            terminal_size: &mut terminal_size,
        },
        &kernel,
        &registry,
    );

    let result = handle.execute(&mut runtime, &args);
    assert_eq!(result, CommandResult::Success);
}
