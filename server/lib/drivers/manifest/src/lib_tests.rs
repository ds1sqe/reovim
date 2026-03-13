use super::*;

const VALID_TOML: &str = r#"
[personality]
name = "vim"

[[keybinding]]
key = "<C-y>"
command = "completion:confirm"
modes = ["vim:insert"]
category = "completion"
description = "Confirm completion"

[[keybinding]]
key = "gd"
command = "lsp-navigation:goto-definition"
modes = ["vim:normal"]
category = "lsp"
description = "Go to definition (LSP)"

[[mode-bridge]]
feature_mode = "snippet:navigating"
parent_mode = "vim:insert"
"#;

const MINIMAL_TOML: &str = r#"
[personality]
name = "test"
"#;

// ========== Parsing ==========

#[test]
fn parse_valid_manifest() {
    let manifest = PersonalityManifest::parse(VALID_TOML).unwrap();
    assert_eq!(manifest.personality.name, "vim");
    assert_eq!(manifest.keybindings.len(), 2);
    assert_eq!(manifest.mode_bridges.len(), 1);
}

#[test]
fn parse_minimal_manifest() {
    let manifest = PersonalityManifest::parse(MINIMAL_TOML).unwrap();
    assert_eq!(manifest.personality.name, "test");
    assert!(manifest.keybindings.is_empty());
    assert!(manifest.mode_bridges.is_empty());
}

#[test]
fn parse_error_invalid_toml() {
    let result = PersonalityManifest::parse("this is not valid toml [[[");
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.to_string().contains("manifest parse error"));
}

#[test]
fn parse_error_missing_personality() {
    let result = PersonalityManifest::parse("[[keybinding]]\nkey = \"a\"\n");
    assert!(result.is_err());
}

#[test]
fn parse_error_missing_required_fields() {
    let toml = r#"
[personality]
name = "test"

[[keybinding]]
key = "a"
"#;
    let result = PersonalityManifest::parse(toml);
    assert!(result.is_err());
}

#[test]
fn parse_keybinding_fields() {
    let manifest = PersonalityManifest::parse(VALID_TOML).unwrap();
    let kb = &manifest.keybindings[0];
    assert_eq!(kb.key, "<C-y>");
    assert_eq!(kb.command, "completion:confirm");
    assert_eq!(kb.modes, vec!["vim:insert"]);
    assert_eq!(kb.category, "completion");
    assert_eq!(kb.description, "Confirm completion");
}

#[test]
fn parse_mode_bridge_fields() {
    let manifest = PersonalityManifest::parse(VALID_TOML).unwrap();
    let bridge = &manifest.mode_bridges[0];
    assert_eq!(bridge.feature_mode, "snippet:navigating");
    assert_eq!(bridge.parent_mode, "vim:insert");
}

// ========== Conversion ==========

#[test]
fn to_keybinding_registrations_count() {
    let manifest = PersonalityManifest::parse(VALID_TOML).unwrap();
    let regs = manifest.to_keybinding_registrations();
    assert_eq!(regs.len(), 2);
}

#[test]
fn to_keybinding_registrations_keys() {
    let manifest = PersonalityManifest::parse(VALID_TOML).unwrap();
    let regs = manifest.to_keybinding_registrations();
    assert_eq!(regs[0].keys, "<C-y>");
    assert_eq!(regs[1].keys, "gd");
}

#[test]
fn to_keybinding_registrations_command_ids() {
    let manifest = PersonalityManifest::parse(VALID_TOML).unwrap();
    let regs = manifest.to_keybinding_registrations();
    assert_eq!(regs[0].command_id.module().as_str(), "completion");
    assert_eq!(regs[0].command_id.name(), "confirm");
    assert_eq!(regs[1].command_id.module().as_str(), "lsp-navigation");
    assert_eq!(regs[1].command_id.name(), "goto-definition");
}

#[test]
fn to_keybinding_registrations_modes() {
    let manifest = PersonalityManifest::parse(VALID_TOML).unwrap();
    let regs = manifest.to_keybinding_registrations();
    assert_eq!(regs[0].modes, &["vim:insert"]);
    assert_eq!(regs[1].modes, &["vim:normal"]);
}

#[test]
fn to_keybinding_registrations_description_and_category() {
    let manifest = PersonalityManifest::parse(VALID_TOML).unwrap();
    let regs = manifest.to_keybinding_registrations();
    assert_eq!(regs[0].description, "Confirm completion");
    assert_eq!(regs[0].category, Some("completion"));
    assert_eq!(regs[1].description, "Go to definition (LSP)");
    assert_eq!(regs[1].category, Some("lsp"));
}

#[test]
fn to_keybinding_registrations_multiple_modes() {
    let toml = r#"
[personality]
name = "test"

[[keybinding]]
key = "x"
command = "editor:delete"
modes = ["vim:normal", "vim:visual"]
category = "edit"
description = "Delete"
"#;
    let manifest = PersonalityManifest::parse(toml).unwrap();
    let regs = manifest.to_keybinding_registrations();
    assert_eq!(regs[0].modes.len(), 2);
    assert_eq!(regs[0].modes[0], "vim:normal");
    assert_eq!(regs[0].modes[1], "vim:visual");
}

#[test]
fn to_keybinding_registrations_empty() {
    let manifest = PersonalityManifest::parse(MINIMAL_TOML).unwrap();
    let regs = manifest.to_keybinding_registrations();
    assert!(regs.is_empty());
}

// ========== Validation: modes ==========

#[test]
fn validate_modes_all_found() {
    let manifest = PersonalityManifest::parse(VALID_TOML).unwrap();
    let known = &[("vim", "insert"), ("vim", "normal")];
    let warnings = manifest.validate_modes(known);
    assert!(warnings.is_empty());
}

#[test]
fn validate_modes_missing() {
    let manifest = PersonalityManifest::parse(VALID_TOML).unwrap();
    let known = &[("vim", "normal")]; // missing vim:insert
    let warnings = manifest.validate_modes(known);
    assert_eq!(warnings.len(), 1);
    assert_eq!(
        warnings[0],
        ManifestWarning::ModeNotFound {
            key: "<C-y>".to_string(),
            mode: "vim:insert".to_string(),
        }
    );
}

#[test]
fn validate_modes_no_colon() {
    let toml = r#"
[personality]
name = "test"

[[keybinding]]
key = "x"
command = "editor:delete"
modes = ["badmode"]
category = "edit"
description = "Delete"
"#;
    let manifest = PersonalityManifest::parse(toml).unwrap();
    let warnings = manifest.validate_modes(&[]);
    assert_eq!(warnings.len(), 1);
    assert!(matches!(
        &warnings[0],
        ManifestWarning::ModeNotFound { mode, .. } if mode == "badmode"
    ));
}

// ========== Validation: commands ==========

#[test]
fn validate_commands_all_loaded() {
    let manifest = PersonalityManifest::parse(VALID_TOML).unwrap();
    let loaded = &[ModuleId::new("completion"), ModuleId::new("lsp-navigation")];
    let warnings = manifest.validate_commands(loaded);
    assert!(warnings.is_empty());
}

#[test]
fn validate_commands_module_not_loaded() {
    let manifest = PersonalityManifest::parse(VALID_TOML).unwrap();
    let loaded = &[ModuleId::new("completion")]; // missing lsp-navigation
    let warnings = manifest.validate_commands(loaded);
    assert_eq!(warnings.len(), 1);
    assert_eq!(
        warnings[0],
        ManifestWarning::ModuleNotLoaded {
            key: "gd".to_string(),
            command: "lsp-navigation:goto-definition".to_string(),
            module: "lsp-navigation".to_string(),
        }
    );
}

#[test]
fn validate_commands_no_colon_in_command() {
    let toml = r#"
[personality]
name = "test"

[[keybinding]]
key = "x"
command = "barecommand"
modes = ["vim:normal"]
category = "misc"
description = "Test"
"#;
    let manifest = PersonalityManifest::parse(toml).unwrap();
    let loaded = &[ModuleId::new("barecommand")];
    let warnings = manifest.validate_commands(loaded);
    assert!(warnings.is_empty());
}

// ========== Conflict detection ==========

#[test]
fn detect_conflicts_none() {
    let manifest = PersonalityManifest::parse(VALID_TOML).unwrap();
    let warnings = manifest.detect_conflicts();
    assert!(warnings.is_empty());
}

#[test]
fn detect_conflicts_found() {
    let toml = r#"
[personality]
name = "test"

[[keybinding]]
key = "<C-y>"
command = "completion:confirm"
modes = ["vim:insert"]
category = "completion"
description = "Confirm completion"

[[keybinding]]
key = "<C-y>"
command = "other:action"
modes = ["vim:insert"]
category = "other"
description = "Other action"
"#;
    let manifest = PersonalityManifest::parse(toml).unwrap();
    let warnings = manifest.detect_conflicts();
    assert_eq!(warnings.len(), 1);
    assert!(matches!(
        &warnings[0],
        ManifestWarning::KeyConflict { key, mode, command_a, command_b }
        if key == "<C-y>" && mode == "vim:insert"
            && command_a == "completion:confirm" && command_b == "other:action"
    ));
}

#[test]
fn detect_conflicts_different_modes_no_conflict() {
    let toml = r#"
[personality]
name = "test"

[[keybinding]]
key = "x"
command = "a:a"
modes = ["vim:normal"]
category = "a"
description = "A"

[[keybinding]]
key = "x"
command = "b:b"
modes = ["vim:insert"]
category = "b"
description = "B"
"#;
    let manifest = PersonalityManifest::parse(toml).unwrap();
    let warnings = manifest.detect_conflicts();
    assert!(warnings.is_empty());
}

// ========== Filtered registrations ==========

#[test]
fn to_filtered_registrations_all_loaded() {
    let manifest = PersonalityManifest::parse(VALID_TOML).unwrap();
    let loaded = &[ModuleId::new("completion"), ModuleId::new("lsp-navigation")];
    let regs = manifest.to_filtered_registrations(loaded);
    assert_eq!(regs.len(), 2);
}

#[test]
fn to_filtered_registrations_partial() {
    let manifest = PersonalityManifest::parse(VALID_TOML).unwrap();
    let loaded = &[ModuleId::new("completion")];
    let regs = manifest.to_filtered_registrations(loaded);
    assert_eq!(regs.len(), 1);
    assert_eq!(regs[0].keys, "<C-y>");
}

#[test]
fn to_filtered_registrations_none_loaded() {
    let manifest = PersonalityManifest::parse(VALID_TOML).unwrap();
    let regs = manifest.to_filtered_registrations(&[]);
    assert!(regs.is_empty());
}

// ========== ModeBridgeStore ==========

#[test]
fn mode_bridge_store_find_parent() {
    let bridges = vec![
        ManifestModeBridge {
            feature_mode: "snippet:navigating".to_string(),
            parent_mode: "vim:insert".to_string(),
        },
        ManifestModeBridge {
            feature_mode: "range-finder:jump-input".to_string(),
            parent_mode: "vim:normal".to_string(),
        },
    ];
    let store = ModeBridgeStore::new(bridges);

    assert_eq!(store.find_parent("snippet:navigating"), Some("vim:insert"));
    assert_eq!(store.find_parent("range-finder:jump-input"), Some("vim:normal"));
    assert_eq!(store.find_parent("unknown:mode"), None);
}

#[test]
fn mode_bridge_store_bridges() {
    let bridges = vec![ManifestModeBridge {
        feature_mode: "snippet:navigating".to_string(),
        parent_mode: "vim:insert".to_string(),
    }];
    let store = ModeBridgeStore::new(bridges);
    assert_eq!(store.bridges().len(), 1);
}

#[test]
fn mode_bridge_store_empty() {
    let store = ModeBridgeStore::new(vec![]);
    assert!(store.find_parent("any:mode").is_none());
    assert!(store.bridges().is_empty());
}

#[test]
fn mode_bridge_store_debug() {
    let store = ModeBridgeStore::new(vec![ManifestModeBridge {
        feature_mode: "a:b".to_string(),
        parent_mode: "c:d".to_string(),
    }]);
    let debug = format!("{store:?}");
    assert!(debug.contains("ModeBridgeStore"));
    assert!(debug.contains('1'));
}

// ========== Error display ==========

#[test]
fn manifest_error_display() {
    let err = ManifestError::Parse("bad toml".to_string());
    assert_eq!(err.to_string(), "manifest parse error: bad toml");
}

#[test]
fn manifest_error_is_error() {
    let err = ManifestError::Parse("test".to_string());
    let _: &dyn std::error::Error = &err;
}

#[test]
fn manifest_error_debug() {
    let err = ManifestError::Parse("test".to_string());
    let debug = format!("{err:?}");
    assert!(debug.contains("Parse"));
}

// ========== Warning display ==========

#[test]
fn warning_mode_not_found_display() {
    let w = ManifestWarning::ModeNotFound {
        key: "x".to_string(),
        mode: "vim:insert".to_string(),
    };
    assert_eq!(w.to_string(), "keybinding 'x': mode 'vim:insert' not found");
}

#[test]
fn warning_module_not_loaded_display() {
    let w = ManifestWarning::ModuleNotLoaded {
        key: "gd".to_string(),
        command: "lsp:goto".to_string(),
        module: "lsp".to_string(),
    };
    assert!(w.to_string().contains("unloaded module 'lsp'"));
}

#[test]
fn warning_key_conflict_display() {
    let w = ManifestWarning::KeyConflict {
        key: "x".to_string(),
        mode: "vim:normal".to_string(),
        command_a: "a:a".to_string(),
        command_b: "b:b".to_string(),
    };
    assert!(w.to_string().contains("key conflict"));
}

// ========== Roundtrip ==========

#[test]
fn roundtrip_parse_to_registrations() {
    let toml = r#"
[personality]
name = "vim"

[[keybinding]]
key = "<Space>f"
command = "microscope:open-files"
modes = ["vim:normal"]
category = "picker"
description = "Open file picker"

[[mode-bridge]]
feature_mode = "range-finder:jump-input"
parent_mode = "vim:normal"
"#;
    let manifest = PersonalityManifest::parse(toml).unwrap();
    assert_eq!(manifest.personality.name, "vim");

    let regs = manifest.to_keybinding_registrations();
    assert_eq!(regs.len(), 1);
    assert_eq!(regs[0].keys, "<Space>f");
    assert_eq!(regs[0].command_id.module().as_str(), "microscope");
    assert_eq!(regs[0].command_id.name(), "open-files");
    assert_eq!(regs[0].modes, &["vim:normal"]);
    assert_eq!(regs[0].description, "Open file picker");
    assert_eq!(regs[0].category, Some("picker"));

    assert_eq!(manifest.mode_bridges.len(), 1);
    assert_eq!(manifest.mode_bridges[0].feature_mode, "range-finder:jump-input");
    assert_eq!(manifest.mode_bridges[0].parent_mode, "vim:normal");
}

// ========== Option parsing (#610) ==========

const OPTIONS_TOML: &str = r#"
[personality]
name = "test"

[[option]]
name = "scrolloff"
short = "so"
description = "Minimum lines above/below cursor"
default = 0
constraint = { min = 0 }

[[option]]
name = "number"
short = "nu"
description = "Show line numbers"
scope = "window"
default = false

[[option]]
name = "wrapscan"
short = "ws"
description = "Wrap search around end of file"
default = true
"#;

#[test]
fn parse_options_count() {
    let manifest = PersonalityManifest::parse(OPTIONS_TOML).unwrap();
    assert_eq!(manifest.options.len(), 3);
}

#[test]
fn parse_option_integer_fields() {
    let manifest = PersonalityManifest::parse(OPTIONS_TOML).unwrap();
    let opt = &manifest.options[0];
    assert_eq!(opt.name, "scrolloff");
    assert_eq!(opt.short, Some("so".to_string()));
    assert_eq!(opt.description, "Minimum lines above/below cursor");
    assert_eq!(opt.scope, ManifestOptionScope::Global);
    assert_eq!(opt.default, ManifestOptionValue::Integer(0));
    assert_eq!(
        opt.constraint,
        ManifestConstraint {
            min: Some(0),
            max: None,
            min_length: None,
            max_length: None,
        }
    );
}

#[test]
fn parse_option_bool_window_scope() {
    let manifest = PersonalityManifest::parse(OPTIONS_TOML).unwrap();
    let opt = &manifest.options[1];
    assert_eq!(opt.name, "number");
    assert_eq!(opt.scope, ManifestOptionScope::Window);
    assert_eq!(opt.default, ManifestOptionValue::Bool(false));
}

#[test]
fn parse_option_bool_true_default() {
    let manifest = PersonalityManifest::parse(OPTIONS_TOML).unwrap();
    let opt = &manifest.options[2];
    assert_eq!(opt.name, "wrapscan");
    assert_eq!(opt.default, ManifestOptionValue::Bool(true));
}

#[test]
fn parse_option_string_default() {
    let toml = r#"
[personality]
name = "test"

[[option]]
name = "theme"
description = "Color theme"
default = "monokai"
"#;
    let manifest = PersonalityManifest::parse(toml).unwrap();
    let opt = &manifest.options[0];
    assert_eq!(opt.default, ManifestOptionValue::String("monokai".to_string()));
}

#[test]
fn parse_option_choice_default() {
    let toml = r#"
[personality]
name = "test"

[[option]]
name = "virtualedit"
description = "Allow cursor beyond end of line"
default = { value = "none", choices = ["none", "all", "block"] }
"#;
    let manifest = PersonalityManifest::parse(toml).unwrap();
    let opt = &manifest.options[0];
    assert_eq!(
        opt.default,
        ManifestOptionValue::Choice {
            value: "none".to_string(),
            choices: vec!["none".to_string(), "all".to_string(), "block".to_string()],
        }
    );
}

#[test]
fn parse_option_buffer_scope() {
    let toml = r#"
[personality]
name = "test"

[[option]]
name = "filetype"
description = "Buffer file type"
scope = "buffer"
default = ""
"#;
    let manifest = PersonalityManifest::parse(toml).unwrap();
    let opt = &manifest.options[0];
    assert_eq!(opt.scope, ManifestOptionScope::Buffer);
}

#[test]
fn parse_option_range_constraint() {
    let toml = r#"
[personality]
name = "test"

[[option]]
name = "tabstop"
description = "Tab width"
scope = "buffer"
default = 4
constraint = { min = 1, max = 32 }
"#;
    let manifest = PersonalityManifest::parse(toml).unwrap();
    let opt = &manifest.options[0];
    assert_eq!(
        opt.constraint,
        ManifestConstraint {
            min: Some(1),
            max: Some(32),
            min_length: None,
            max_length: None,
        }
    );
}

#[test]
fn parse_option_string_length_constraint() {
    let toml = r#"
[personality]
name = "test"

[[option]]
name = "label"
description = "Label"
default = "x"
constraint = { min_length = 1, max_length = 10 }
"#;
    let manifest = PersonalityManifest::parse(toml).unwrap();
    let opt = &manifest.options[0];
    assert_eq!(
        opt.constraint,
        ManifestConstraint {
            min: None,
            max: None,
            min_length: Some(1),
            max_length: Some(10),
        }
    );
}

#[test]
fn parse_no_options_defaults_to_empty() {
    let manifest = PersonalityManifest::parse(MINIMAL_TOML).unwrap();
    assert!(manifest.options.is_empty());
}

#[test]
fn parse_existing_toml_with_no_options() {
    // Existing VALID_TOML (no options) should still parse with empty options
    let manifest = PersonalityManifest::parse(VALID_TOML).unwrap();
    assert!(manifest.options.is_empty());
    // Other fields still work
    assert_eq!(manifest.keybindings.len(), 2);
    assert_eq!(manifest.mode_bridges.len(), 1);
}

#[test]
fn parse_option_default_scope_is_global() {
    // When scope is omitted, it defaults to global
    let toml = r#"
[personality]
name = "test"

[[option]]
name = "hlsearch"
description = "Highlight search"
default = false
"#;
    let manifest = PersonalityManifest::parse(toml).unwrap();
    assert_eq!(manifest.options[0].scope, ManifestOptionScope::Global);
}

#[test]
fn parse_option_default_constraint_is_none() {
    // When constraint is omitted, all fields are None
    let toml = r#"
[personality]
name = "test"

[[option]]
name = "hlsearch"
description = "Highlight search"
default = false
"#;
    let manifest = PersonalityManifest::parse(toml).unwrap();
    assert_eq!(manifest.options[0].constraint, ManifestConstraint::default());
}

#[test]
fn parse_option_no_short_form() {
    let toml = r#"
[personality]
name = "test"

[[option]]
name = "noshort"
description = "No short form"
default = false
"#;
    let manifest = PersonalityManifest::parse(toml).unwrap();
    assert!(manifest.options[0].short.is_none());
}

// ========== Option conversion (#610) ==========

use reovim_kernel::api::v1::{OptionConstraint, OptionScope, OptionValue};

#[test]
fn to_option_specs_count() {
    let manifest = PersonalityManifest::parse(OPTIONS_TOML).unwrap();
    let specs = manifest.to_option_specs(&ModuleId::new("vim"));
    assert_eq!(specs.len(), 3);
}

#[test]
fn to_option_specs_names() {
    let manifest = PersonalityManifest::parse(OPTIONS_TOML).unwrap();
    let specs = manifest.to_option_specs(&ModuleId::new("vim"));
    assert_eq!(specs[0].name.as_ref(), "scrolloff");
    assert_eq!(specs[1].name.as_ref(), "number");
    assert_eq!(specs[2].name.as_ref(), "wrapscan");
}

#[test]
fn to_option_specs_short_forms() {
    let manifest = PersonalityManifest::parse(OPTIONS_TOML).unwrap();
    let specs = manifest.to_option_specs(&ModuleId::new("vim"));
    assert_eq!(specs[0].short_form.as_deref(), Some("so"));
    assert_eq!(specs[1].short_form.as_deref(), Some("nu"));
    assert_eq!(specs[2].short_form.as_deref(), Some("ws"));
}

#[test]
fn to_option_specs_descriptions() {
    let manifest = PersonalityManifest::parse(OPTIONS_TOML).unwrap();
    let specs = manifest.to_option_specs(&ModuleId::new("vim"));
    assert_eq!(specs[0].description.as_ref(), "Minimum lines above/below cursor");
}

#[test]
fn to_option_specs_defaults() {
    let manifest = PersonalityManifest::parse(OPTIONS_TOML).unwrap();
    let specs = manifest.to_option_specs(&ModuleId::new("vim"));
    assert_eq!(specs[0].default, OptionValue::int(0));
    assert_eq!(specs[1].default, OptionValue::bool(false));
    assert_eq!(specs[2].default, OptionValue::bool(true));
}

#[test]
fn to_option_specs_scopes() {
    let manifest = PersonalityManifest::parse(OPTIONS_TOML).unwrap();
    let specs = manifest.to_option_specs(&ModuleId::new("vim"));
    assert_eq!(specs[0].scope, OptionScope::Global);
    assert_eq!(specs[1].scope, OptionScope::Window);
    assert_eq!(specs[2].scope, OptionScope::Global);
}

#[test]
fn to_option_specs_constraints() {
    let manifest = PersonalityManifest::parse(OPTIONS_TOML).unwrap();
    let specs = manifest.to_option_specs(&ModuleId::new("vim"));
    assert_eq!(specs[0].constraint, OptionConstraint::min(0));
    assert_eq!(specs[1].constraint, OptionConstraint::none());
}

#[test]
fn to_option_specs_owner() {
    let owner = ModuleId::new("vim");
    let manifest = PersonalityManifest::parse(OPTIONS_TOML).unwrap();
    let specs = manifest.to_option_specs(&owner);
    for spec in &specs {
        assert_eq!(spec.owner(), Some(&owner));
    }
}

#[test]
fn to_option_specs_no_short_form() {
    let toml = r#"
[personality]
name = "test"

[[option]]
name = "noshort"
description = "No short form"
default = false
"#;
    let manifest = PersonalityManifest::parse(toml).unwrap();
    let specs = manifest.to_option_specs(&ModuleId::new("test"));
    assert!(specs[0].short_form.is_none());
}

#[test]
fn to_option_specs_string_value() {
    let toml = r#"
[personality]
name = "test"

[[option]]
name = "theme"
description = "Theme"
default = "monokai"
"#;
    let manifest = PersonalityManifest::parse(toml).unwrap();
    let specs = manifest.to_option_specs(&ModuleId::new("test"));
    assert_eq!(specs[0].default, OptionValue::string("monokai"));
}

#[test]
fn to_option_specs_choice_value() {
    let toml = r#"
[personality]
name = "test"

[[option]]
name = "virtualedit"
description = "Virtual edit mode"
default = { value = "none", choices = ["none", "all", "block"] }
"#;
    let manifest = PersonalityManifest::parse(toml).unwrap();
    let specs = manifest.to_option_specs(&ModuleId::new("test"));
    assert_eq!(
        specs[0].default,
        OptionValue::choice(
            "none",
            vec!["none".to_string(), "all".to_string(), "block".to_string()]
        )
    );
}

#[test]
fn to_option_specs_buffer_scope() {
    let toml = r#"
[personality]
name = "test"

[[option]]
name = "tabstop"
description = "Tab width"
scope = "buffer"
default = 4
"#;
    let manifest = PersonalityManifest::parse(toml).unwrap();
    let specs = manifest.to_option_specs(&ModuleId::new("test"));
    assert_eq!(specs[0].scope, OptionScope::Buffer);
}

#[test]
fn to_option_specs_range_constraint() {
    let toml = r#"
[personality]
name = "test"

[[option]]
name = "tabstop"
description = "Tab width"
default = 4
constraint = { min = 1, max = 32 }
"#;
    let manifest = PersonalityManifest::parse(toml).unwrap();
    let specs = manifest.to_option_specs(&ModuleId::new("test"));
    assert_eq!(specs[0].constraint, OptionConstraint::range(1, 32));
}

#[test]
fn to_option_specs_empty() {
    let manifest = PersonalityManifest::parse(MINIMAL_TOML).unwrap();
    let specs = manifest.to_option_specs(&ModuleId::new("test"));
    assert!(specs.is_empty());
}

#[test]
fn to_option_spec_single_integer() {
    let spec_manifest = ManifestOptionSpec {
        name: "scrolloff".to_string(),
        short: Some("so".to_string()),
        description: "Scroll offset".to_string(),
        scope: ManifestOptionScope::Global,
        default: ManifestOptionValue::Integer(5),
        constraint: ManifestConstraint {
            min: Some(0),
            max: Some(999),
            min_length: None,
            max_length: None,
        },
    };
    let spec = spec_manifest.to_option_spec(&ModuleId::new("vim"));
    assert_eq!(spec.name.as_ref(), "scrolloff");
    assert_eq!(spec.short_form.as_deref(), Some("so"));
    assert_eq!(spec.default, OptionValue::int(5));
    assert_eq!(spec.scope, OptionScope::Global);
    assert_eq!(spec.constraint, OptionConstraint::range(0, 999));
    assert_eq!(spec.owner().map(ModuleId::as_str), Some("vim"));
}

#[test]
fn manifest_option_value_debug() {
    let val = ManifestOptionValue::Bool(true);
    let debug = format!("{val:?}");
    assert!(debug.contains("Bool"));
}

#[test]
fn manifest_option_scope_debug() {
    let scope = ManifestOptionScope::Window;
    let debug = format!("{scope:?}");
    assert!(debug.contains("Window"));
}

#[test]
fn manifest_constraint_debug() {
    let c = ManifestConstraint {
        min: Some(0),
        max: None,
        min_length: None,
        max_length: None,
    };
    let debug = format!("{c:?}");
    assert!(debug.contains("min"));
}

#[test]
fn manifest_option_scope_default_is_global() {
    assert_eq!(ManifestOptionScope::default(), ManifestOptionScope::Global);
}

#[test]
fn manifest_constraint_default_is_none() {
    let c = ManifestConstraint::default();
    assert!(c.min.is_none());
    assert!(c.max.is_none());
    assert!(c.min_length.is_none());
    assert!(c.max_length.is_none());
}

#[test]
fn manifest_option_value_clone() {
    let val = ManifestOptionValue::Integer(42);
    let cloned = val.clone();
    assert_eq!(val, cloned);
}

#[test]
fn manifest_option_spec_clone() {
    let spec = ManifestOptionSpec {
        name: "test".to_string(),
        short: None,
        description: "Test".to_string(),
        scope: ManifestOptionScope::Global,
        default: ManifestOptionValue::Bool(false),
        constraint: ManifestConstraint::default(),
    };
    let cloned = spec.clone();
    assert_eq!(spec, cloned);
}

#[test]
fn manifest_constraint_clone() {
    let c = ManifestConstraint {
        min: Some(1),
        max: Some(100),
        min_length: None,
        max_length: None,
    };
    let cloned = c.clone();
    assert_eq!(c, cloned);
}

#[test]
fn manifest_option_scope_copy() {
    let scope = ManifestOptionScope::Buffer;
    let copied = scope;
    assert_eq!(scope, copied);
}

#[test]
fn to_option_spec_string_length_constraint() {
    let spec_manifest = ManifestOptionSpec {
        name: "label".to_string(),
        short: None,
        description: "Label".to_string(),
        scope: ManifestOptionScope::Global,
        default: ManifestOptionValue::String("x".to_string()),
        constraint: ManifestConstraint {
            min: None,
            max: None,
            min_length: Some(1),
            max_length: Some(10),
        },
    };
    let spec = spec_manifest.to_option_spec(&ModuleId::new("test"));
    assert_eq!(spec.constraint, OptionConstraint::string_length(1, 10));
}
