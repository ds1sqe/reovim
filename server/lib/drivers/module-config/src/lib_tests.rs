use std::error::Error;

use super::*;

const FULL_TOML: &str = r#"
extends = "official"

[modules.tetromino]
enabled = false

[modules.lsp]
enabled = true
[modules.lsp.settings]
auto_start = true
servers = ["rust-analyzer", "pyright"]

[modules.completion]
enabled = true
[modules.completion.settings]
max_items = 20

[extensions.polyblocks]
enabled = false
"#;

const MINIMAL_TOML: &str = r#"
extends = "official"
"#;

// ============================================================================
// Parsing
// ============================================================================

#[test]
fn parse_valid_full_config() {
    let config = ModulesConfig::parse(FULL_TOML).unwrap();
    assert_eq!(config.extends, "official");
    assert_eq!(config.modules.len(), 3);
    assert_eq!(config.extensions.len(), 1);
}

#[test]
fn parse_minimal_config() {
    let config = ModulesConfig::parse(MINIMAL_TOML).unwrap();
    assert_eq!(config.extends, "official");
    assert!(config.modules.is_empty());
    assert!(config.extensions.is_empty());
}

#[test]
fn parse_empty_string_defaults_to_official() {
    let config = ModulesConfig::parse("").unwrap();
    assert_eq!(config.extends, "official");
    assert!(config.modules.is_empty());
    assert!(config.extensions.is_empty());
}

#[test]
fn parse_error_invalid_toml() {
    let result = ModulesConfig::parse("this is not valid toml [[[");
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.to_string().contains("module config parse error"));
}

#[test]
fn parse_module_entry_without_settings() {
    let toml = r"
[modules.tetromino]
enabled = false
";
    let config = ModulesConfig::parse(toml).unwrap();
    let entry = &config.modules["tetromino"];
    assert!(!entry.enabled);
    assert!(entry.settings.is_none());
}

#[test]
fn parse_settings_with_nested_tables() {
    let toml = r#"
[modules.lsp]
enabled = true
[modules.lsp.settings]
auto_start = true
[modules.lsp.settings.server]
name = "rust-analyzer"
args = ["--log-file", "/tmp/ra.log"]
"#;
    let config = ModulesConfig::parse(toml).unwrap();
    let settings = config.module_settings("lsp").unwrap();
    assert!(settings.get("server").is_some());
    assert!(settings.get("auto_start").is_some());
}

#[test]
fn parse_empty_modules_section() {
    let toml = r#"
extends = "official"
[modules]
"#;
    let config = ModulesConfig::parse(toml).unwrap();
    assert!(config.modules.is_empty());
}

#[test]
fn parse_empty_extensions_section() {
    let toml = r#"
extends = "official"
[extensions]
"#;
    let config = ModulesConfig::parse(toml).unwrap();
    assert!(config.extensions.is_empty());
}

#[test]
fn parse_module_entry_enabled_defaults_to_true() {
    let toml = r"
[modules.lsp]
[modules.lsp.settings]
auto_start = true
";
    let config = ModulesConfig::parse(toml).unwrap();
    assert!(config.modules["lsp"].enabled);
}

// ============================================================================
// Load from file
// ============================================================================

#[test]
fn load_nonexistent_file_returns_official() {
    let config = ModulesConfig::load(Path::new("/nonexistent/path/modules.toml")).unwrap();
    assert_eq!(config, ModulesConfig::official());
}

#[test]
fn load_valid_file() {
    let dir =
        std::env::temp_dir().join(format!("reovim-module-config-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("modules.toml");
    std::fs::write(&path, "[modules.tetromino]\nenabled = false\n").unwrap();

    let config = ModulesConfig::load(&path).unwrap();
    assert!(!config.is_module_enabled("tetromino"));

    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn load_invalid_file_returns_parse_error() {
    let dir =
        std::env::temp_dir().join(format!("reovim-module-config-invalid-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("modules.toml");
    std::fs::write(&path, "this is not valid [[[").unwrap();

    let result = ModulesConfig::load(&path);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("parse error"));

    std::fs::remove_dir_all(&dir).unwrap();
}

// ============================================================================
// Official preset
// ============================================================================

#[test]
fn official_preset_has_no_disabled_modules() {
    let config = ModulesConfig::official();
    assert!(config.disabled_modules().is_empty());
    assert!(config.disabled_extensions().is_empty());
    assert_eq!(config.extends, "official");
}

// ============================================================================
// Enable/disable queries
// ============================================================================

#[test]
fn is_module_enabled_true_for_unlisted() {
    let config = ModulesConfig::official();
    assert!(config.is_module_enabled("vim"));
    assert!(config.is_module_enabled("anything"));
}

#[test]
fn is_module_enabled_false_for_disabled() {
    let config = ModulesConfig::parse(FULL_TOML).unwrap();
    assert!(!config.is_module_enabled("tetromino"));
}

#[test]
fn is_module_enabled_true_for_explicit_enabled() {
    let config = ModulesConfig::parse(FULL_TOML).unwrap();
    assert!(config.is_module_enabled("lsp"));
}

#[test]
fn is_extension_enabled_true_for_unlisted() {
    let config = ModulesConfig::official();
    assert!(config.is_extension_enabled("completion"));
}

#[test]
fn is_extension_enabled_false_for_disabled() {
    let config = ModulesConfig::parse(FULL_TOML).unwrap();
    assert!(!config.is_extension_enabled("polyblocks"));
}

// ============================================================================
// Module settings
// ============================================================================

#[test]
fn module_settings_extracts_correctly() {
    let config = ModulesConfig::parse(FULL_TOML).unwrap();
    let settings = config.module_settings("lsp").unwrap();
    assert_eq!(settings.get("auto_start").unwrap().as_bool(), Some(true));
    let servers = settings.get("servers").unwrap().as_array().unwrap();
    assert_eq!(servers.len(), 2);
}

#[test]
fn module_settings_returns_none_for_no_settings() {
    let config = ModulesConfig::parse(FULL_TOML).unwrap();
    assert!(config.module_settings("tetromino").is_none());
}

#[test]
fn module_settings_returns_none_for_unlisted() {
    let config = ModulesConfig::official();
    assert!(config.module_settings("vim").is_none());
}

// ============================================================================
// Disabled lists
// ============================================================================

#[test]
fn disabled_modules_lists_only_disabled() {
    let config = ModulesConfig::parse(FULL_TOML).unwrap();
    let disabled = config.disabled_modules();
    assert_eq!(disabled.len(), 1);
    assert!(disabled.contains(&"tetromino"));
}

#[test]
fn disabled_extensions_lists_only_disabled() {
    let config = ModulesConfig::parse(FULL_TOML).unwrap();
    let disabled = config.disabled_extensions();
    assert_eq!(disabled.len(), 1);
    assert!(disabled.contains(&"polyblocks"));
}

#[test]
fn disabled_modules_empty_for_official() {
    let config = ModulesConfig::official();
    assert!(config.disabled_modules().is_empty());
}

// ============================================================================
// Validation
// ============================================================================

#[test]
fn validate_modules_warns_on_unknown() {
    let config = ModulesConfig::parse(FULL_TOML).unwrap();
    let known = &["vim", "lsp", "completion"];
    let warnings = config.validate_modules(known);
    assert_eq!(warnings.len(), 1);
    assert!(warnings.iter().any(|w| matches!(
        w,
        ModuleConfigWarning::UnknownModule { id } if id == "tetromino"
    )));
}

#[test]
fn validate_modules_empty_when_all_known() {
    let config = ModulesConfig::parse(FULL_TOML).unwrap();
    let known = &["tetromino", "lsp", "completion"];
    let warnings = config.validate_modules(known);
    assert!(warnings.is_empty());
}

#[test]
fn validate_modules_multiple_warnings() {
    let toml = r"
[modules.foo]
enabled = false
[modules.bar]
enabled = false
";
    let config = ModulesConfig::parse(toml).unwrap();
    let warnings = config.validate_modules(&[]);
    assert_eq!(warnings.len(), 2);
}

#[test]
fn validate_modules_empty_known_list() {
    let config = ModulesConfig::parse(FULL_TOML).unwrap();
    let warnings = config.validate_modules(&[]);
    assert_eq!(warnings.len(), 3); // tetromino, lsp, completion
}

#[test]
fn validate_extensions_warns_on_unknown() {
    let config = ModulesConfig::parse(FULL_TOML).unwrap();
    let known = &["completion"];
    let warnings = config.validate_extensions(known);
    assert_eq!(warnings.len(), 1);
    assert!(warnings.iter().any(|w| matches!(
        w,
        ModuleConfigWarning::UnknownExtension { kind } if kind == "polyblocks"
    )));
}

#[test]
fn validate_extensions_empty_when_all_known() {
    let config = ModulesConfig::parse(FULL_TOML).unwrap();
    let known = &["polyblocks"];
    let warnings = config.validate_extensions(known);
    assert!(warnings.is_empty());
}

// ============================================================================
// Error Display/Debug
// ============================================================================

#[test]
fn module_config_error_parse_display() {
    let err = ModuleConfigError::Parse("bad toml".to_string());
    assert_eq!(err.to_string(), "module config parse error: bad toml");
}

#[test]
fn module_config_error_io_display() {
    let io_err = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "access denied");
    let err = ModuleConfigError::Io(io_err);
    assert!(err.to_string().contains("IO error"));
    assert!(err.to_string().contains("access denied"));
}

#[test]
fn module_config_error_debug() {
    let err = ModuleConfigError::Parse("test".to_string());
    let debug = format!("{err:?}");
    assert!(debug.contains("Parse"));
}

#[test]
fn module_config_error_is_std_error() {
    let err = ModuleConfigError::Parse("test".to_string());
    let _: &dyn std::error::Error = &err;
}

#[test]
fn module_config_error_io_source() {
    let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "not found");
    let err = ModuleConfigError::Io(io_err);
    assert!(err.source().is_some());
}

#[test]
fn module_config_error_parse_source() {
    let err = ModuleConfigError::Parse("test".to_string());
    assert!(err.source().is_none());
}

// ============================================================================
// Warning Display
// ============================================================================

#[test]
fn warning_unknown_module_display() {
    let w = ModuleConfigWarning::UnknownModule {
        id: "foo".to_string(),
    };
    assert_eq!(w.to_string(), "unknown module 'foo' in config");
}

#[test]
fn warning_unknown_extension_display() {
    let w = ModuleConfigWarning::UnknownExtension {
        kind: "bar".to_string(),
    };
    assert_eq!(w.to_string(), "unknown extension 'bar' in config");
}

#[test]
fn warning_dependency_conflict_display() {
    let w = ModuleConfigWarning::DependencyConflict {
        disabled: "vim".to_string(),
        required_by: "motions".to_string(),
    };
    let msg = w.to_string();
    assert!(msg.contains("disabled module 'vim'"));
    assert!(msg.contains("required by enabled module 'motions'"));
}

// ============================================================================
// ModuleConfigStore
// ============================================================================

#[test]
fn config_store_get_returns_settings() {
    let mut configs = HashMap::new();
    configs.insert("lsp".to_string(), toml::Value::Boolean(true));
    let store = ModuleConfigStore::new(configs);
    assert_eq!(store.get("lsp"), Some(&toml::Value::Boolean(true)));
}

#[test]
fn config_store_get_returns_none_for_missing() {
    let store = ModuleConfigStore::new(HashMap::new());
    assert!(store.get("anything").is_none());
}

#[test]
fn config_store_is_empty_true() {
    let store = ModuleConfigStore::new(HashMap::new());
    assert!(store.is_empty());
}

#[test]
fn config_store_is_empty_false() {
    let mut configs = HashMap::new();
    configs.insert("x".to_string(), toml::Value::Boolean(true));
    let store = ModuleConfigStore::new(configs);
    assert!(!store.is_empty());
}

#[test]
fn config_store_debug() {
    let store = ModuleConfigStore::new(HashMap::new());
    let debug = format!("{store:?}");
    assert!(debug.contains("ModuleConfigStore"));
    assert!(debug.contains('0'));
}

// ============================================================================
// PartialEq on config types
// ============================================================================

#[test]
fn modules_config_equality() {
    let a = ModulesConfig::official();
    let b = ModulesConfig::official();
    assert_eq!(a, b);
}

#[test]
fn module_entry_equality() {
    let a = ModuleEntry {
        enabled: false,
        settings: None,
    };
    let b = ModuleEntry {
        enabled: false,
        settings: None,
    };
    assert_eq!(a, b);
}

#[test]
fn extension_entry_equality() {
    let a = ExtensionEntry { enabled: false };
    let b = ExtensionEntry { enabled: false };
    assert_eq!(a, b);
}

// ============================================================================
// build_config_store
// ============================================================================

#[test]
fn build_config_store_empty_for_official() {
    let config = ModulesConfig::official();
    let store = config.build_config_store();
    assert!(store.is_empty());
}

#[test]
fn build_config_store_extracts_settings() {
    let config = ModulesConfig::parse(FULL_TOML).unwrap();
    let store = config.build_config_store();
    // lsp and completion have settings, tetromino does not
    assert!(store.get("lsp").is_some());
    assert!(store.get("completion").is_some());
    assert!(store.get("tetromino").is_none());
}

#[test]
fn build_config_store_settings_values_correct() {
    let config = ModulesConfig::parse(FULL_TOML).unwrap();
    let store = config.build_config_store();
    let lsp = store.get("lsp").unwrap();
    assert_eq!(lsp.get("auto_start").unwrap().as_bool(), Some(true));
}

#[test]
fn build_config_store_no_settings_entries() {
    let toml = r"
[modules.foo]
enabled = false
[modules.bar]
enabled = true
";
    let config = ModulesConfig::parse(toml).unwrap();
    let store = config.build_config_store();
    assert!(store.is_empty());
}
