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
