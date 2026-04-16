use {
    super::*,
    reovim_kernel::api::v1::ModuleId,
    reovim_subsys_command::Command,
};

// Test command implementation (metadata-only, no CommandHandler)
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

    fn names(&self) -> &[&'static str] {
        std::slice::from_ref(&self.name)
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

// execute_for_client: REMOVED (#753 E6). Command execution routes through DomainDriver.
// get_handle / HandlerBridge: REMOVED (#753 E6). No longer in CommandRegistry.

#[test]
fn test_command_registry_unregister_for_module() {
    let mut registry = CommandRegistry::new();
    let owner = ModuleId::new("my-module");

    registry.register_for_module(Arc::new(TestCommand::new("cmd1")), owner.clone());
    registry.register_for_module(Arc::new(TestCommand::new("cmd2")), owner.clone());
    registry.register(Arc::new(TestCommand::new("cmd3")));

    assert_eq!(registry.len(), 3);

    let removed = registry.unregister_for_module(&owner);

    assert_eq!(removed, 2);
    assert_eq!(registry.len(), 1);
}

#[test]
fn test_command_registry_replace_existing() {
    let mut registry = CommandRegistry::new();
    let id = CommandId::new(ModuleId::new("test"), "same-cmd");

    registry.register(Arc::new(TestCommand::new("same-cmd")));
    assert_eq!(registry.len(), 1);

    registry.register(Arc::new(TestCommand::new("same-cmd")));
    assert_eq!(registry.len(), 1);
    assert!(registry.contains(&id));
}

// ========================================================================
// CommandPriority tests (#545)
// ========================================================================

struct PriorityTestCommand {
    id: CommandId,
    name: &'static str,
    priority: CommandPriority,
    desc: &'static str,
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl PriorityTestCommand {
    fn normal(name: &'static str) -> Self {
        Self {
            id: CommandId::new(ModuleId::new("test"), name),
            name,
            priority: CommandPriority::Normal,
            desc: "Normal priority",
        }
    }

    fn override_priority(name: &'static str) -> Self {
        Self {
            id: CommandId::new(ModuleId::new("test"), name),
            name,
            priority: CommandPriority::Override,
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
    fn priority(&self) -> CommandPriority {
        self.priority
    }
}

#[test]
fn test_override_priority_wins_over_normal() {
    let mut registry = CommandRegistry::new();
    let id = CommandId::new(ModuleId::new("test"), "cmd");

    registry.register(Arc::new(PriorityTestCommand::normal("cmd")));
    assert_eq!(registry.get(&id).unwrap().description(), "Normal priority");

    registry.register(Arc::new(PriorityTestCommand::override_priority("cmd")));
    assert_eq!(registry.get(&id).unwrap().description(), "Override priority");
}

#[test]
fn test_normal_cannot_replace_override() {
    let mut registry = CommandRegistry::new();
    let id = CommandId::new(ModuleId::new("test"), "cmd");

    registry.register(Arc::new(PriorityTestCommand::override_priority("cmd")));
    assert_eq!(registry.get(&id).unwrap().description(), "Override priority");

    registry.register(Arc::new(PriorityTestCommand::normal("cmd")));
    assert_eq!(registry.get(&id).unwrap().description(), "Override priority");
    assert_eq!(registry.len(), 1);
}

#[test]
fn test_equal_priority_last_wins() {
    let mut registry = CommandRegistry::new();
    let id = CommandId::new(ModuleId::new("test"), "cmd");

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

    registry.register(Arc::new(PriorityTestCommand::normal("cmd")));

    registry.register_for_module(Arc::new(PriorityTestCommand::override_priority("cmd")), owner);
    assert_eq!(registry.get(&id).unwrap().description(), "Override priority");

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
            &[]
        }
    }

    let mut registry = CommandRegistry::new();
    registry.register(Arc::new(TestCommand::new("visible")));
    registry.register(Arc::new(InternalCommand));

    let snapshot = CommandQuerySnapshot::from_registry(&registry);
    assert_eq!(snapshot.count(), 2);
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

#[test]
fn test_build_name_index_from_registry() {
    let mut registry = CommandRegistry::new();
    registry.register(Arc::new(MultiNameCommand::new("write", &["w", "write"])));
    registry.register(Arc::new(MultiNameCommand::new("quit", &["q", "quit"])));

    let index = registry.build_name_index();
    assert_eq!(index.count(), 2);

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
    let mut registry = CommandRegistry::new();
    registry.register(Arc::new(TestCommand::new("internal-cmd")));

    let index = registry.build_name_index();
    assert!(index.resolve("internal-cmd").is_some());
}
