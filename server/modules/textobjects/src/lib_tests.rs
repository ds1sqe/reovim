use super::*;

#[test]
fn test_module_trait() {
    let module = TextObjectsModule;
    assert_eq!(module.id().as_str(), "textobjects");
    assert_eq!(module.name(), "Vim Text Objects");
}

#[test]
fn test_module_version() {
    let module = TextObjectsModule;
    let version = module.version();
    assert_eq!(version.major, 0);
    assert_eq!(version.minor, 9);
    assert_eq!(version.patch, 0);
}

#[test]
fn test_module_init() {
    use {
        reovim_kernel::api::v1::{KernelContext, ModuleContext, ServiceRegistry},
        std::{path::PathBuf, sync::Arc},
    };

    let kernel = KernelContext::default();
    let services = Arc::new(ServiceRegistry::new());
    let ctx = ModuleContext::new(
        kernel,
        services.clone(),
        PathBuf::from("/tmp/test-data"),
        PathBuf::from("/tmp/test-cache"),
    );

    let mut module = TextObjectsModule::new();
    let result = module.init(&ctx);
    assert_eq!(result, ProbeResult::Success);

    // Verify commands were registered in CommandHandlerStore
    let store = services.get::<CommandHandlerStore>();
    assert!(store.is_some(), "CommandHandlerStore should be registered");
    let handlers = store.unwrap().take_handlers();
    assert_eq!(handlers.len(), 34, "All 34 text object commands should be registered");
}

#[test]
fn test_command_provider_trait() {
    let module = TextObjectsModule::new();
    let handlers = module.command_handlers();
    assert_eq!(handlers.len(), all_commands().len());
}

#[test]
fn test_module_exit() {
    let mut module = TextObjectsModule::new();
    let result = module.exit();
    assert!(result.is_ok());
}

#[test]
fn test_module_default() {
    let module = TextObjectsModule;
    assert_eq!(module.name(), "Vim Text Objects");
}

#[test]
fn test_module_new() {
    let _module = TextObjectsModule::new();
}

#[test]
fn test_textobjects_module_id_constant() {
    assert_eq!(TEXTOBJECTS_MODULE.as_str(), "textobjects");
}

#[test]
fn test_word_commands_count() {
    let cmds = word::all_commands();
    assert_eq!(cmds.len(), 4); // iw, aw, iW, aW
}

#[test]
fn test_quote_commands_count() {
    let cmds = quote::all_commands();
    assert_eq!(cmds.len(), 6); // i", a", i', a', i`, a`
}

#[test]
fn test_bracket_commands_count() {
    let cmds = bracket::all_commands();
    assert_eq!(cmds.len(), 8); // i(, a(, i[, a[, i{, a{, i<, a<
}

#[test]
fn test_paragraph_commands_count() {
    let cmds = paragraph::all_commands();
    assert_eq!(cmds.len(), 2); // ip, ap
}

#[test]
fn test_all_commands_count() {
    let cmds = all_commands();
    assert_eq!(cmds.len(), 34); // 4 + 6 + 8 + 2 + 14
}

#[test]
fn test_module_default_impl() {
    fn make_default<T: Default>() -> T {
        T::default()
    }
    let module: TextObjectsModule = make_default();
    assert_eq!(module.id().as_str(), "textobjects");
}

#[test]
fn test_all_commands_have_valid_ids() {
    let cmds = all_commands();
    for cmd in &cmds {
        assert_eq!(cmd.id().module().as_str(), "textobjects");
        assert!(!cmd.id().name().is_empty());
    }
}

#[test]
fn test_all_commands_have_descriptions() {
    let cmds = all_commands();
    for cmd in &cmds {
        assert!(!cmd.description().is_empty());
    }
}

#[test]
fn test_all_commands_args_valid() {
    let cmds = all_commands();
    for cmd in &cmds {
        // All commands should return a valid args vec (empty is fine for semantic textobjects)
        let _args = cmd.args();
    }
}

#[test]
fn test_semantic_commands_count() {
    let cmds = semantic::all_commands();
    assert_eq!(cmds.len(), 14); // 7 kinds x 2 scopes
}
