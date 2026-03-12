use std::collections::HashSet;

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
    // Code intelligence modules (2): lsp, lsp-navigation
    // Snippet (1): snippet
    // Range-finder (1): range-finder
    // Completion (1): completion
    // Explorer (1): explorer
    // Tetromino (1): tetromino
    // Total: 29 modules
    // Note: vim adapter modules removed (#585) — personality manifests handle bridging
    // Note: picker-options added (#599)
    assert_eq!(deps.len(), 29);
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
    // Code intelligence modules (2): lsp, lsp-navigation
    // Snippet (1): snippet
    // Range-finder (1): range-finder
    // Completion (1): completion
    // Explorer (1): explorer
    // Tetromino (1): tetromino
    // Total: 29 modules
    // Note: vim adapter modules removed (#585) — personality manifests handle bridging
    // Note: picker-options added (#599)
    assert_eq!(modules.len(), 29);
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

// ============================================================================
// Builtin Registry
// ============================================================================

#[test]
fn builtin_registry_has_29_entries() {
    let registry = DefaultsModule::builtin_registry();
    assert_eq!(registry.len(), 29);
}

#[test]
fn builtin_registry_keys_match_order() {
    let registry = DefaultsModule::builtin_registry();
    let order = DefaultsModule::builtin_order();
    let registry_keys: HashSet<&str> = registry.keys().copied().collect();
    let order_keys: HashSet<&str> = order.iter().copied().collect();
    assert_eq!(registry_keys, order_keys);
}

#[test]
fn builtin_order_has_29_entries() {
    let order = DefaultsModule::builtin_order();
    assert_eq!(order.len(), 29);
}

#[test]
fn builtin_order_has_no_duplicates() {
    let order = DefaultsModule::builtin_order();
    let unique: HashSet<&str> = order.iter().copied().collect();
    assert_eq!(order.len(), unique.len());
}

#[test]
fn builtin_order_matches_create_modules() {
    let order = DefaultsModule::builtin_order();
    let modules = DefaultsModule::create_modules();
    assert_eq!(order.len(), modules.len());
    for (id, module) in order.iter().zip(modules.iter()) {
        assert_eq!(*id, module.id().as_str());
    }
}

#[test]
fn builtin_registry_factories_produce_correct_ids() {
    let registry = DefaultsModule::builtin_registry();
    for (id, factory) in &registry {
        let module = factory();
        assert_eq!(
            *id,
            module.id().as_str(),
            "Factory for '{id}' produced module with id '{}'",
            module.id().as_str()
        );
    }
}

// ============================================================================
// Filtered Module Creation
// ============================================================================

#[test]
fn create_modules_filtered_all_enabled() {
    let modules = DefaultsModule::create_modules_filtered(|_| true);
    assert_eq!(modules.len(), 29);
}

#[test]
fn create_modules_filtered_all_disabled() {
    let modules = DefaultsModule::create_modules_filtered(|_| false);
    assert!(modules.is_empty());
}

#[test]
fn create_modules_filtered_single_module() {
    let modules = DefaultsModule::create_modules_filtered(|id| id == "vim");
    assert_eq!(modules.len(), 1);
    assert_eq!(modules[0].id().as_str(), "vim");
}

#[test]
fn create_modules_filtered_preserves_order() {
    let modules =
        DefaultsModule::create_modules_filtered(|id| id == "undo" || id == "vim" || id == "lsp");
    assert_eq!(modules.len(), 3);
    assert_eq!(modules[0].id().as_str(), "undo");
    assert_eq!(modules[1].id().as_str(), "vim");
    assert_eq!(modules[2].id().as_str(), "lsp");
}

#[test]
fn create_modules_filtered_disable_one() {
    let modules = DefaultsModule::create_modules_filtered(|id| id != "tetromino");
    assert_eq!(modules.len(), 28);
    assert!(modules.iter().all(|m| m.id().as_str() != "tetromino"));
}

#[test]
fn create_modules_filtered_has_unique_ids() {
    let modules = DefaultsModule::create_modules_filtered(|_| true);
    let ids: Vec<String> = modules
        .iter()
        .map(|m| m.id().as_str().to_string())
        .collect();
    let unique: HashSet<&str> = ids.iter().map(String::as_str).collect();
    assert_eq!(ids.len(), unique.len());
}

#[test]
fn create_modules_is_equivalent_to_filtered_all() {
    let unfiltered = DefaultsModule::create_modules();
    let filtered = DefaultsModule::create_modules_filtered(|_| true);
    assert_eq!(unfiltered.len(), filtered.len());
    for (a, b) in unfiltered.iter().zip(filtered.iter()) {
        assert_eq!(a.id(), b.id());
    }
}
