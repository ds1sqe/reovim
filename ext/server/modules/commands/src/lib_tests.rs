use {super::*, reovim_driver_command::Command};

// ========================================================================
// command_handlers() function tests
// ========================================================================

#[test]
fn test_command_handlers_count() {
    let cmds = command_handlers();
    assert_eq!(cmds.len(), 15); // 8 base (incl. mount) + 3 session + 4 buffer
}

#[test]
fn test_command_handlers_contains_all_ids() {
    let cmds = command_handlers();
    let ids: Vec<_> = cmds.iter().map(|c| c.id().name_owned()).collect();
    assert!(ids.iter().any(|n| n == "edit"));
    assert!(ids.iter().any(|n| n == "quit"));
    assert!(ids.iter().any(|n| n == "write"));
    assert!(ids.iter().any(|n| n == "write-quit"));
    assert!(ids.iter().any(|n| n == "colorscheme"));
    assert!(ids.iter().any(|n| n == "set"));
    assert!(ids.iter().any(|n| n == "detach"));
    assert!(ids.iter().any(|n| n == "servers"));
    assert!(ids.iter().any(|n| n == "kill-server"));
    assert!(ids.iter().any(|n| n == "substitute"));
    assert!(ids.iter().any(|n| n == "bnext"));
    assert!(ids.iter().any(|n| n == "bprevious"));
    assert!(ids.iter().any(|n| n == "bdelete"));
    assert!(ids.iter().any(|n| n == "qall"));
}

#[test]
fn test_command_handlers_unique_ids() {
    let cmds = command_handlers();
    let ids: Vec<_> = cmds.iter().map(|c| c.id()).collect();
    for (i, id) in ids.iter().enumerate() {
        for (j, other) in ids.iter().enumerate() {
            if i != j {
                assert_ne!(id, other, "duplicate command id: {}", id.name());
            }
        }
    }
}

// ========================================================================
// Command type re-export tests
// ========================================================================

#[test]
fn test_quit_command_names() {
    let cmd = QuitCommand;
    let names = cmd.names();
    assert!(names.contains(&"q"));
    assert!(names.contains(&"quit"));
}

#[test]
fn test_write_command_names() {
    let cmd = WriteCommand;
    let names = cmd.names();
    assert!(names.contains(&"w"));
    assert!(names.contains(&"write"));
}

#[test]
fn test_session_commands_re_export() {
    let detach = DetachCommand;
    let servers = ServersCommand;
    let kill = KillServerCommand;
    assert_eq!(detach.id().name(), "detach");
    assert_eq!(servers.id().name(), "servers");
    assert_eq!(kill.id().name(), "kill-server");
}

// ========================================================================
// CommandsModule tests
// ========================================================================

#[test]
fn test_module_id() {
    let module = CommandsModule;
    assert_eq!(module.id().as_str(), "commands");
}

#[test]
fn test_module_name() {
    let module = CommandsModule;
    assert_eq!(module.name(), "Ex-Commands");
}

#[test]
fn test_module_version() {
    let module = CommandsModule;
    let version = module.version();
    assert_eq!(version.major, 0);
    assert_eq!(version.minor, 9);
    assert_eq!(version.patch, 0);
}

#[test]
fn test_module_new() {
    let module = CommandsModule::new();
    assert_eq!(module.id().as_str(), "commands");
}

#[test]
fn test_module_default() {
    fn accepts_default<T: Default>(val: T) -> T {
        drop(val);
        T::default()
    }
    let module = accepts_default(CommandsModule);
    assert_eq!(module.id().as_str(), "commands");
}

#[test]
fn test_module_exit() {
    let mut module = CommandsModule::new();
    let result = module.exit();
    assert!(result.is_ok());
}

#[test]
fn test_module_init_registers_commands() {
    use reovim_kernel::api::v1::ModuleContext;

    let mut module = CommandsModule::new();
    let ctx = ModuleContext::default();

    let result = module.init(&ctx);
    assert_eq!(result, ProbeResult::Success);

    // Verify commands were registered in the CommandHandlerStore
    let store = ctx.services.get::<CommandHandlerStore>().unwrap();
    assert_eq!(store.len(), 15);
}

#[test]
fn test_module_init_idempotent_stores_accumulate() {
    use reovim_kernel::api::v1::ModuleContext;

    let mut module = CommandsModule::new();
    let ctx = ModuleContext::default();

    // Init twice - handlers accumulate in the store
    module.init(&ctx);
    module.init(&ctx);

    let store = ctx.services.get::<CommandHandlerStore>().unwrap();
    assert_eq!(store.len(), 30); // 15 + 15
}
