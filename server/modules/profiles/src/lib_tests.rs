use super::*;

#[test]
fn test_command_handlers_count() {
    let cmds = command_handlers(PathBuf::from("/test"));
    assert_eq!(cmds.len(), 3);
}

#[test]
fn test_command_handlers_unique_ids() {
    let cmds = command_handlers(PathBuf::from("/test"));
    let ids: Vec<_> = cmds.iter().map(|c| c.id()).collect();
    for (i, id) in ids.iter().enumerate() {
        for (j, other) in ids.iter().enumerate() {
            if i != j {
                assert_ne!(id, other, "duplicate command id: {}", id.name());
            }
        }
    }
}

#[test]
fn test_command_handlers_contains_all_ids() {
    let cmds = command_handlers(PathBuf::from("/test"));
    let ids: Vec<_> = cmds.iter().map(|c| c.id().name_owned()).collect();
    assert!(ids.iter().any(|n| n == "profile-save"));
    assert!(ids.iter().any(|n| n == "profile-load"));
    assert!(ids.iter().any(|n| n == "profile-list"));
}

#[test]
fn test_module_id() {
    let module = ProfilesModule;
    assert_eq!(module.id().as_str(), "profiles");
}

#[test]
fn test_module_name() {
    let module = ProfilesModule;
    assert_eq!(module.name(), "Configuration Profiles");
}

#[test]
fn test_module_version() {
    let module = ProfilesModule;
    let version = module.version();
    assert_eq!(version.major, 0);
    assert_eq!(version.minor, 10);
    assert_eq!(version.patch, 0);
}

#[test]
fn test_module_new() {
    let module = ProfilesModule::new();
    assert_eq!(module.id().as_str(), "profiles");
}

#[test]
fn test_module_default() {
    fn accepts_default<T: Default>(val: T) -> T {
        drop(val);
        T::default()
    }
    let module = accepts_default(ProfilesModule);
    assert_eq!(module.id().as_str(), "profiles");
}

#[test]
fn test_module_exit() {
    let mut module = ProfilesModule::new();
    let result = module.exit();
    assert!(result.is_ok());
}

#[test]
fn test_module_init_registers_commands() {
    let mut module = ProfilesModule::new();
    let ctx = ModuleContext::default();

    let result = module.init(&ctx);
    assert_eq!(result, ProbeResult::Success);

    let store = ctx.services.get::<CommandHandlerStore>().unwrap();
    assert_eq!(store.len(), 3);
}
