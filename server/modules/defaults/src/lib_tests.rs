use super::*;

#[test]
fn test_defaults_module_id() {
    let module = DefaultsModule::new();
    assert_eq!(module.id().as_str(), "defaults");
}

#[test]
fn test_defaults_module_name() {
    let module = DefaultsModule::new();
    assert_eq!(module.name(), "Default Modules Bundle");
}

#[test]
fn test_defaults_has_dependencies() {
    let module = DefaultsModule::new();
    let deps = module.dependencies();
    // Service modules (6): undo, buffer-simple, search, scratch-buffer, vfs-local, clipboard
    // Utility modules (2): keymap, commands
    // Policy modules (3): editor, motions, vim
    // Extension bridge modules (3): cmdline, whichkey, notification
    // Picker providers (5): picker-files, picker-buffers, picker-commands, picker-grep, picker-options
    // Picker orchestration (1): microscope
    // Syntax modules (2): treesitter-rust, treesitter-markdown
    // Code intelligence modules (3): lsp, lsp-navigation, vim-lsp
    // Snippet (1): snippet
    // Range-finder (1): range-finder
    // Completion (1): completion
    // Explorer (1): explorer
    // Vim adapter modules (5): vim-microscope, vim-explorer, vim-completion, vim-range-finder, vim-snippet
    // Tetromino (1): tetromino
    // Total: 35 modules
    assert_eq!(deps.len(), 35);
}

#[test]
fn test_create_modules() {
    let modules = DefaultsModule::create_modules();
    // Service modules (6): undo, buffer-simple, search, scratch-buffer, vfs-local, clipboard
    // Utility modules (2): keymap, commands
    // Policy modules (3): editor, motions, vim
    // Extension bridge modules (3): cmdline, whichkey, notification
    // Picker providers (5): picker-files, picker-buffers, picker-commands, picker-grep, picker-options
    // Picker orchestration (1): microscope
    // Syntax modules (2): treesitter-rust, treesitter-markdown
    // Code intelligence modules (3): lsp, lsp-navigation, vim-lsp
    // Snippet (1): snippet
    // Range-finder (1): range-finder
    // Completion (1): completion
    // Explorer (1): explorer
    // Vim adapter modules (5): vim-microscope, vim-explorer, vim-completion, vim-range-finder, vim-snippet
    // Tetromino (1): tetromino
    // Total: 35 modules
    assert_eq!(modules.len(), 35);
}

#[test]
fn test_operators_not_empty() {
    let ops = operators();
    assert!(!ops.is_empty());
}

#[test]
fn test_command_handlers_not_empty() {
    let cmds = command_handlers();
    assert!(!cmds.is_empty());
}

#[test]
fn test_keybindings_not_empty() {
    let bindings = keybindings();
    assert!(!bindings.is_empty());
}

#[test]
fn test_defaults_module_version() {
    let module = DefaultsModule::new();
    let version = module.version();
    assert_eq!(version.major, 0);
    assert_eq!(version.minor, 9);
    assert_eq!(version.patch, 0);
}

#[test]
fn test_defaults_module_default() {
    fn create_default<T: Default>() -> T {
        T::default()
    }
    let from_default: DefaultsModule = create_default();
    let from_new = DefaultsModule::new();
    assert_eq!(from_new.id(), from_default.id());
    assert_eq!(from_new.version(), from_default.version());
}

#[test]
fn test_exit_succeeds() {
    let mut module = DefaultsModule::new();
    assert!(module.exit().is_ok());
}

#[test]
fn test_dependencies_contain_service_modules() {
    let module = DefaultsModule::new();
    let deps = module.dependencies();
    let dep_strs: Vec<&str> = deps.iter().map(ModuleId::as_str).collect();

    assert!(dep_strs.contains(&"undo"));
    assert!(dep_strs.contains(&"buffer-simple"));
    assert!(dep_strs.contains(&"search"));
    assert!(dep_strs.contains(&"scratch-buffer"));
    assert!(dep_strs.contains(&"vfs-local"));
    assert!(dep_strs.contains(&"clipboard"));
}

#[test]
fn test_dependencies_contain_utility_modules() {
    let module = DefaultsModule::new();
    let deps = module.dependencies();
    let dep_strs: Vec<&str> = deps.iter().map(ModuleId::as_str).collect();

    assert!(dep_strs.contains(&"keymap"));
    assert!(dep_strs.contains(&"commands"));
}

#[test]
fn test_dependencies_contain_policy_modules() {
    let module = DefaultsModule::new();
    let deps = module.dependencies();
    let dep_strs: Vec<&str> = deps.iter().map(ModuleId::as_str).collect();

    assert!(dep_strs.contains(&"editor"));
    assert!(dep_strs.contains(&"motions"));
    assert!(dep_strs.contains(&"vim"));
}

#[test]
fn test_dependencies_contain_syntax_modules() {
    let module = DefaultsModule::new();
    let deps = module.dependencies();
    let dep_strs: Vec<&str> = deps.iter().map(ModuleId::as_str).collect();

    assert!(dep_strs.contains(&"treesitter-rust"));
    assert!(dep_strs.contains(&"treesitter-markdown"));
}

#[test]
fn test_dependencies_contain_lsp_module() {
    let module = DefaultsModule::new();
    let deps = module.dependencies();
    let dep_strs: Vec<&str> = deps.iter().map(ModuleId::as_str).collect();

    assert!(dep_strs.contains(&"lsp"));
    assert!(dep_strs.contains(&"lsp-navigation"));
    assert!(dep_strs.contains(&"vim-lsp"));
}

#[test]
fn test_dependencies_contain_vim_adapter_modules() {
    let module = DefaultsModule::new();
    let deps = module.dependencies();
    let dep_strs: Vec<&str> = deps.iter().map(ModuleId::as_str).collect();

    assert!(dep_strs.contains(&"vim-microscope"));
    assert!(dep_strs.contains(&"vim-explorer"));
    assert!(dep_strs.contains(&"vim-completion"));
    assert!(dep_strs.contains(&"vim-range-finder"));
    assert!(dep_strs.contains(&"vim-snippet"));
}

#[test]
fn test_create_modules_has_unique_ids() {
    let modules = DefaultsModule::create_modules();
    let ids: Vec<String> = modules
        .iter()
        .map(|m| m.id().as_str().to_string())
        .collect();

    // Check all IDs are unique
    let mut deduped = ids.clone();
    deduped.sort();
    deduped.dedup();
    assert_eq!(ids.len(), deduped.len(), "Module IDs should all be unique, found duplicates");
}

#[test]
fn test_keybindings_from_module_trait() {
    let module = DefaultsModule::new();
    let bindings = module.keybindings();
    // Should be same as the free function
    assert_eq!(bindings.len(), keybindings().len());
}

#[test]
fn test_init_returns_success() {
    use {
        reovim_kernel::api::v1::{KernelContext, ModuleContext, ServiceRegistry},
        std::{path::PathBuf, sync::Arc},
    };

    let kernel = KernelContext::default();
    let services = Arc::new(ServiceRegistry::new());
    let ctx = ModuleContext::new(
        kernel,
        services,
        PathBuf::from("/tmp/test-data"),
        PathBuf::from("/tmp/test-cache"),
    );

    let mut module = DefaultsModule::new();
    let result = module.init(&ctx);
    assert_eq!(result, ProbeResult::Success);
}
