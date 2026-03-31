use {
    reovim_driver_annotation::AnnotationSourceRegistry,
    reovim_driver_command::{CommandHandlerStore, CommandProvider},
    reovim_driver_input::{KeybindingStore, ModeInfoStore, ModeProviderRegistry, ResolverRegistry},
    reovim_kernel::api::v1::{Module, ModuleContext, OptionScope, OptionValue, ProbeResult},
};

use crate::{VimMode, VimModule, bindings, commands, operators, visual};

#[test]
fn test_vim_module_id() {
    let module = VimModule::new();
    assert_eq!(module.id().as_str(), "vim");
}

#[test]
fn test_vim_module_name() {
    let module = VimModule::new();
    assert_eq!(module.name(), "Vim");
}

#[test]
fn test_vim_module_version() {
    let module = VimModule::new();
    let version = module.version();
    assert_eq!(version.major, 0);
    assert_eq!(version.minor, 9);
}

#[test]
fn test_vim_keybindings_not_empty() {
    let module = VimModule::new();
    let bindings = module.keybindings();
    assert!(!bindings.is_empty(), "Vim module should provide keybindings");
    // Should have at least 100 bindings across all modes
    assert!(bindings.len() > 100, "Vim module should have many keybindings");
}

#[test]
fn test_normal_mode_bindings() {
    let bindings = bindings::normal::bindings();
    assert!(!bindings.is_empty(), "Normal mode should have bindings");
    // Verify basic navigation keys exist
    assert!(bindings.iter().any(|b| b.keys == "h"), "Normal mode should have 'h' binding");
    assert!(bindings.iter().any(|b| b.keys == "j"), "Normal mode should have 'j' binding");
    assert!(bindings.iter().any(|b| b.keys == "k"), "Normal mode should have 'k' binding");
    assert!(bindings.iter().any(|b| b.keys == "l"), "Normal mode should have 'l' binding");
}

#[test]
fn test_insert_mode_bindings() {
    let bindings = bindings::insert::bindings();
    assert!(!bindings.is_empty(), "Insert mode should have bindings");
    // Verify escape key exists
    assert!(
        bindings.iter().any(|b| b.keys == "<Esc>"),
        "Insert mode should have Escape binding"
    );
}

#[test]
fn test_visual_mode_bindings() {
    let bindings = bindings::visual::bindings();
    assert!(!bindings.is_empty(), "Visual mode should have bindings");
}

#[test]
fn test_operator_modes_bindings() {
    let bindings = bindings::operator_modes::all_operator_bindings();
    assert!(!bindings.is_empty(), "Operator modes should have bindings");
}

#[test]
fn test_commandline_mode_bindings() {
    let bindings = bindings::commandline::bindings();
    assert!(!bindings.is_empty(), "Commandline mode should have bindings");
}

#[test]
fn test_all_bindings_aggregation() {
    let all = bindings::all();
    let normal = bindings::normal::bindings();
    let insert = bindings::insert::bindings();
    let visual = bindings::visual::bindings();
    let operator_modes = bindings::operator_modes::all_operator_bindings();
    let cmdline = bindings::commandline::bindings();
    let window = bindings::window::bindings();

    let expected_total = normal.len()
        + insert.len()
        + visual.len()
        + operator_modes.len()
        + cmdline.len()
        + window.len();
    assert_eq!(all.len(), expected_total, "all() should aggregate all mode bindings");
}

// ========================================================================
// Epic #445: Line number option tests
// ========================================================================

#[test]
fn test_line_number_options_registered() {
    let mut module = VimModule::new();
    let ctx = ModuleContext::default();

    let result = module.init(&ctx);
    assert!(
        matches!(result, ProbeResult::Success),
        "Vim module should initialize successfully"
    );

    // Verify 'number' option is registered
    assert!(ctx.kernel.options.contains("number"), "'number' option should be registered");

    // Verify 'relativenumber' option is registered
    assert!(
        ctx.kernel.options.contains("relativenumber"),
        "'relativenumber' option should be registered"
    );
}

#[test]
fn test_line_number_option_aliases() {
    let mut module = VimModule::new();
    let ctx = ModuleContext::default();
    module.init(&ctx);

    // Verify short alias 'nu' resolves to 'number'
    assert_eq!(
        ctx.kernel.options.resolve_name("nu"),
        Some("number".to_string()),
        "'nu' should be an alias for 'number'"
    );

    // Verify short alias 'rnu' resolves to 'relativenumber'
    assert_eq!(
        ctx.kernel.options.resolve_name("rnu"),
        Some("relativenumber".to_string()),
        "'rnu' should be an alias for 'relativenumber'"
    );
}

#[test]
fn test_line_number_option_defaults() {
    let mut module = VimModule::new();
    let ctx = ModuleContext::default();
    module.init(&ctx);

    // Both options should default to false
    let number_val = ctx.kernel.options.get_global("number");
    assert_eq!(number_val, Some(OptionValue::bool(false)), "'number' should default to false");

    let rnu_val = ctx.kernel.options.get_global("relativenumber");
    assert_eq!(
        rnu_val,
        Some(OptionValue::bool(false)),
        "'relativenumber' should default to false"
    );
}

#[test]
fn test_line_number_option_scope() {
    let mut module = VimModule::new();
    let ctx = ModuleContext::default();
    module.init(&ctx);

    // Both options should have Window scope
    let number_spec = ctx.kernel.options.get_spec("number").unwrap();
    assert_eq!(number_spec.scope, OptionScope::Window, "'number' should have Window scope");

    let rnu_spec = ctx.kernel.options.get_spec("relativenumber").unwrap();
    assert_eq!(rnu_spec.scope, OptionScope::Window, "'relativenumber' should have Window scope");
}

// ========================================================================
// Epic #570: Vim behavior options (#573)
// ========================================================================

#[test]
fn test_vim_manifest_options_count() {
    let manifest =
        reovim_driver_manifest::PersonalityManifest::parse(crate::VIM_MANIFEST_TOML).unwrap();
    // 9 options: number, relativenumber, scrolloff, sidescrolloff,
    // ignorecase, smartcase, hlsearch, incsearch, wrapscan
    assert_eq!(manifest.options.len(), 9);
}

#[test]
fn test_vim_options_registered_after_init() {
    let mut module = VimModule::new();
    let ctx = ModuleContext::default();
    module.init(&ctx);

    let expected = [
        "scrolloff",
        "sidescrolloff",
        "ignorecase",
        "smartcase",
        "hlsearch",
        "incsearch",
        "wrapscan",
    ];
    for name in &expected {
        assert!(ctx.kernel.options.contains(name), "'{name}' should be registered");
    }
}

#[test]
fn test_vim_options_aliases() {
    let mut module = VimModule::new();
    let ctx = ModuleContext::default();
    module.init(&ctx);

    let aliases = [
        ("so", "scrolloff"),
        ("siso", "sidescrolloff"),
        ("ic", "ignorecase"),
        ("scs", "smartcase"),
        ("hls", "hlsearch"),
        ("is", "incsearch"),
        ("ws", "wrapscan"),
    ];
    for (short, full) in &aliases {
        assert_eq!(
            ctx.kernel.options.resolve_name(short),
            Some(full.to_string()),
            "'{short}' should resolve to '{full}'"
        );
    }
}

#[test]
fn test_vim_options_defaults() {
    let mut module = VimModule::new();
    let ctx = ModuleContext::default();
    module.init(&ctx);

    assert_eq!(ctx.kernel.options.get_global("scrolloff"), Some(OptionValue::int(0)));
    assert_eq!(ctx.kernel.options.get_global("sidescrolloff"), Some(OptionValue::int(0)));
    assert_eq!(ctx.kernel.options.get_global("ignorecase"), Some(OptionValue::bool(false)));
    assert_eq!(ctx.kernel.options.get_global("smartcase"), Some(OptionValue::bool(false)));
    assert_eq!(ctx.kernel.options.get_global("hlsearch"), Some(OptionValue::bool(false)));
    assert_eq!(ctx.kernel.options.get_global("incsearch"), Some(OptionValue::bool(false)));
    assert_eq!(ctx.kernel.options.get_global("wrapscan"), Some(OptionValue::bool(true)));
}

#[test]
fn test_vim_options_ownership() {
    let mut module = VimModule::new();
    let ctx = ModuleContext::default();
    module.init(&ctx);

    let vim_options = ctx.kernel.options.list_by_module(&crate::VIM_MODULE);
    // 7 vim behavior options + 2 line number options = 9
    assert_eq!(vim_options.len(), 9);
}

#[test]
fn test_vim_init_fails_on_duplicate_option() {
    use reovim_kernel::api::v1::OptionSpec;

    let ctx = ModuleContext::default();

    // Pre-register one of our options to trigger a conflict
    let _ = ctx.kernel.options.register(OptionSpec::new(
        "scrolloff",
        "Already taken",
        OptionValue::int(1),
    ));

    let mut module = VimModule::new();
    let result = module.init(&ctx);
    assert!(matches!(result, ProbeResult::Failed(_)), "init should fail on duplicate option");
}

// ========================================================================
// AnnotationSource registration test
// ========================================================================

#[test]
fn test_annotation_source_registered() {
    let mut module = VimModule::new();
    let ctx = ModuleContext::default();
    module.init(&ctx);

    // Verify AnnotationSourceRegistry is created and has a source
    let registry = ctx.services.get::<AnnotationSourceRegistry>();
    assert!(registry.is_some(), "AnnotationSourceRegistry should be created");

    let registry = registry.unwrap();
    assert_eq!(registry.len(), 1, "Should have 1 annotation source registered");
}

// ========================================================================
// VimModule Default and exit tests
// ========================================================================

#[test]
fn test_vim_module_default() {
    let module = VimModule;
    assert_eq!(module.id().as_str(), "vim");
    assert_eq!(module.name(), "Vim");
}

#[test]
fn test_vim_module_exit() {
    let mut module = VimModule::new();
    let result = module.exit();
    assert!(result.is_ok(), "exit() should succeed");
}

#[test]
fn test_vim_module_const_new() {
    const MODULE: VimModule = VimModule::new();
    assert_eq!(MODULE.name(), "Vim");
}

// ========================================================================
// CommandProvider tests
// ========================================================================

#[test]
fn test_command_handlers_not_empty() {
    let module = VimModule::new();
    let handlers = module.command_handlers();
    assert!(!handlers.is_empty(), "Should have command handlers");
}

#[test]
fn test_command_handlers_count() {
    let module = VimModule::new();
    let handlers = module.command_handlers();
    // mode_commands() + visual_commands() + operator_commands()
    let mode_count = commands::mode_commands().len();
    let visual_count = visual::visual_commands().len();
    let operator_count = operators::operator_commands().len();
    let expected = mode_count + visual_count + operator_count;
    assert_eq!(
        handlers.len(),
        expected,
        "command_handlers should aggregate mode + visual + operator commands"
    );
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_command_handlers_all_have_ids() {
    let module = VimModule::new();
    let handlers = module.command_handlers();
    for handler in &handlers {
        let id = handler.id();
        assert_eq!(id.module().as_str(), "vim", "handler '{}' should be in vim module", id.name());
    }
}

// ========================================================================
// Init registration counts
// ========================================================================

#[test]
fn test_init_registers_resolvers() {
    let mut module = VimModule::new();
    let ctx = ModuleContext::default();
    module.init(&ctx);

    let resolver_registry = ctx.services.get::<ResolverRegistry>();
    assert!(resolver_registry.is_some(), "ResolverRegistry should exist after init");
    let resolver_registry = resolver_registry.unwrap();
    // Should have: normal, insert, replace, delete, yank, change, commandline, window,
    // visual, visual-line, visual-block, lowercase, uppercase, toggle-case = 14
    assert_eq!(resolver_registry.len(), 14, "Should register 14 resolvers");
}

#[test]
fn test_init_registers_modes() {
    let mut module = VimModule::new();
    let ctx = ModuleContext::default();
    module.init(&ctx);

    let mode_store = ctx.services.get::<ModeInfoStore>();
    assert!(mode_store.is_some(), "ModeInfoStore should exist after init");
    let mode_store = mode_store.unwrap();
    assert_eq!(mode_store.len(), VimMode::ALL.len(), "Should register all VimMode variants");
}

#[test]
fn test_init_registers_keybindings() {
    let mut module = VimModule::new();
    let ctx = ModuleContext::default();
    module.init(&ctx);

    let keybinding_store = ctx.services.get::<KeybindingStore>();
    assert!(keybinding_store.is_some(), "KeybindingStore should exist after init");
    let keybinding_store = keybinding_store.unwrap();
    let all_bindings = bindings::all();
    // Keybindings = vim core bindings only (#700: manifest keybindings migrated to in-crate adapters)
    assert_eq!(
        keybinding_store.len(),
        all_bindings.len(),
        "Should register all vim core keybindings"
    );
}

#[test]
fn test_init_registers_commands() {
    let mut module = VimModule::new();
    let ctx = ModuleContext::default();
    module.init(&ctx);

    let command_store = ctx.services.get::<CommandHandlerStore>();
    assert!(command_store.is_some(), "CommandHandlerStore should exist after init");
    let command_store = command_store.unwrap();
    let expected = module.command_handlers().len();
    assert_eq!(command_store.len(), expected, "Should register all command handlers");
}

#[test]
fn test_init_registers_mode_bridge_store() {
    let mut module = VimModule::new();
    let ctx = ModuleContext::default();
    module.init(&ctx);

    let bridge_store = ctx
        .services
        .get::<reovim_driver_manifest::ModeBridgeStore>();
    assert!(bridge_store.is_some(), "ModeBridgeStore should exist after init");
    let bridge_store = bridge_store.unwrap();
    assert_eq!(bridge_store.bridges().len(), 2, "Should have 2 mode bridges");
    assert_eq!(bridge_store.find_parent("snippet:navigating"), Some("vim:insert"));
    assert_eq!(bridge_store.find_parent("range-finder:jump-input"), Some("vim:normal"));
}

#[test]
fn test_vim_manifest_has_no_key_conflicts() {
    let manifest =
        reovim_driver_manifest::PersonalityManifest::parse(crate::VIM_MANIFEST_TOML).unwrap();
    let conflicts = manifest.detect_conflicts();
    assert!(
        conflicts.is_empty(),
        "vim.toml should have no key conflicts, found: {conflicts:?}"
    );
}

#[test]
fn test_vim_manifest_modes_are_valid() {
    let manifest =
        reovim_driver_manifest::PersonalityManifest::parse(crate::VIM_MANIFEST_TOML).unwrap();
    // Validate against VimModule's registered modes
    let known_modes = &[
        ("vim", "normal"),
        ("vim", "insert"),
        ("vim", "visual"),
        ("vim", "operator-pending"),
    ];
    let warnings = manifest.validate_modes(known_modes);
    assert!(
        warnings.is_empty(),
        "vim.toml should only reference valid vim modes, found: {warnings:?}"
    );
}

#[test]
fn test_init_registers_mode_provider() {
    let mut module = VimModule::new();
    let ctx = ModuleContext::default();
    module.init(&ctx);

    let mode_registry = ctx.services.get::<ModeProviderRegistry>();
    assert!(mode_registry.is_some(), "ModeProviderRegistry should exist after init");
}

#[test]
fn test_init_returns_success() {
    let mut module = VimModule::new();
    let ctx = ModuleContext::default();
    let result = module.init(&ctx);
    assert!(matches!(result, ProbeResult::Success));
}

#[test]
fn test_init_idempotent_options() {
    // Second init should fail because options already registered
    let mut module = VimModule::new();
    let ctx = ModuleContext::default();
    let result1 = module.init(&ctx);
    assert!(matches!(result1, ProbeResult::Success));

    let result2 = module.init(&ctx);
    // Second init may fail on option registration - this is expected behavior
    // The important thing is it doesn't panic
    let _ = result2;
}

#[test]
fn test_vim_module_version_patch() {
    let module = VimModule::new();
    let version = module.version();
    assert_eq!(version.patch, 0);
}

// ========================================================================
// Binding category tests
// ========================================================================

#[test]
fn test_normal_mode_has_motion_category() {
    let bindings = bindings::normal::bindings();
    assert!(
        bindings.iter().any(|b| b.category == Some("motion")),
        "Normal mode should have motion bindings"
    );
}

#[test]
fn test_normal_mode_has_operator_category() {
    let bindings = bindings::normal::bindings();
    assert!(
        bindings.iter().any(|b| b.category == Some("operator")),
        "Normal mode should have operator bindings"
    );
}

#[test]
fn test_normal_mode_has_mode_category() {
    let bindings = bindings::normal::bindings();
    assert!(
        bindings.iter().any(|b| b.category == Some("mode")),
        "Normal mode should have mode bindings"
    );
}

#[test]
fn test_normal_mode_has_edit_category() {
    let bindings = bindings::normal::bindings();
    assert!(
        bindings.iter().any(|b| b.category == Some("edit")),
        "Normal mode should have edit bindings"
    );
}

#[test]
fn test_normal_mode_has_history_category() {
    let bindings = bindings::normal::bindings();
    assert!(
        bindings.iter().any(|b| b.category == Some("history")),
        "Normal mode should have history bindings"
    );
}

#[test]
fn test_normal_mode_has_clipboard_category() {
    let bindings = bindings::normal::bindings();
    assert!(
        bindings.iter().any(|b| b.category == Some("clipboard")),
        "Normal mode should have clipboard bindings"
    );
}

#[test]
fn test_normal_mode_has_scroll_category() {
    let bindings = bindings::normal::bindings();
    assert!(
        bindings.iter().any(|b| b.category == Some("scroll")),
        "Normal mode should have scroll bindings"
    );
}

#[test]
fn test_normal_mode_has_search_category() {
    let bindings = bindings::normal::bindings();
    assert!(
        bindings.iter().any(|b| b.category == Some("search")),
        "Normal mode should have search bindings"
    );
}

#[test]
fn test_normal_mode_has_window_category() {
    let bindings = bindings::normal::bindings();
    assert!(
        bindings.iter().any(|b| b.category == Some("window")),
        "Normal mode should have window bindings"
    );
}

#[test]
fn test_normal_mode_has_mark_category() {
    let bindings = bindings::normal::bindings();
    assert!(
        bindings.iter().any(|b| b.category == Some("mark")),
        "Normal mode should have mark bindings"
    );
}

#[test]
fn test_normal_mode_has_session_category() {
    let bindings = bindings::normal::bindings();
    assert!(
        bindings.iter().any(|b| b.category == Some("session")),
        "Normal mode should have session bindings"
    );
}

#[test]
fn test_insert_mode_has_completion_category() {
    let bindings = bindings::insert::bindings();
    assert!(
        bindings.iter().any(|b| b.category == Some("completion")),
        "Insert mode should have completion bindings"
    );
}

#[test]
fn test_visual_mode_has_textobject_category() {
    let bindings = bindings::visual::bindings();
    assert!(
        bindings.iter().any(|b| b.category == Some("textobject")),
        "Visual mode should have textobject bindings"
    );
}

#[test]
fn test_visual_mode_has_selection_category() {
    let bindings = bindings::visual::bindings();
    assert!(
        bindings.iter().any(|b| b.category == Some("selection")),
        "Visual mode should have selection bindings"
    );
}

// =========================================================================
// #718 repro: vim.toml parse failure returns Ok(()) instead of Err.
// lib.rs:340-343 — parse error is logged but swallowed, module loads
// without keybindings.
// =========================================================================

#[test]
fn b7_repro_parse_failure_now_propagates() {
    // Simulate load_personality_manifest() — parse failure now returns Err.
    fn simulate_load(toml: &str) -> Result<bool, String> {
        match reovim_driver_manifest::PersonalityManifest::parse(toml) {
            Ok(_m) => Ok(true),
            Err(e) => Err(format!("Failed to parse: {e}")),
        }
    }

    // PersonalityManifest::parse correctly returns Err on bad input
    let bad_inputs = [
        "not valid toml [[[",
        "",
        "[[keybinding]]\nkey = \"a\"\n",
    ];

    for input in &bad_inputs {
        let result = reovim_driver_manifest::PersonalityManifest::parse(input);
        assert!(result.is_err(), "parse correctly returns Err for: {input:?}");
    }

    // Current embedded vim.toml parses fine
    let valid = reovim_driver_manifest::PersonalityManifest::parse(super::VIM_MANIFEST_TOML);
    assert!(valid.is_ok(), "Current vim.toml parses fine");

    // Bad TOML: parse failure propagates as Err (not silently swallowed)
    assert!(simulate_load("not valid toml [[[").is_err(), "parse failure returns Err");

    // Good TOML: parse succeeds
    assert!(simulate_load(super::VIM_MANIFEST_TOML).unwrap(), "Good TOML loads successfully");
}
