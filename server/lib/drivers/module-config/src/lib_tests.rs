use std::{error::Error, fmt::Write as _};

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

// ============================================================================
// ConfigFieldError
// ============================================================================

#[test]
fn config_field_error_display() {
    let err = ConfigFieldError {
        module_id: "lsp".to_string(),
        field: "timeout".to_string(),
        expected: "integer",
        actual: r#""abc""#.to_string(),
    };
    assert_eq!(err.to_string(), r#"module 'lsp': field 'timeout' expected integer, got "abc""#);
}

#[test]
fn config_field_error_debug() {
    let err = ConfigFieldError {
        module_id: "lsp".to_string(),
        field: "timeout".to_string(),
        expected: "integer",
        actual: "true".to_string(),
    };
    let debug = format!("{err:?}");
    assert!(debug.contains("ConfigFieldError"));
    assert!(debug.contains("lsp"));
}

#[test]
fn config_field_error_is_std_error() {
    let err = ConfigFieldError {
        module_id: "x".to_string(),
        field: "y".to_string(),
        expected: "bool",
        actual: "42".to_string(),
    };
    let _: &dyn std::error::Error = &err;
}

#[test]
fn config_field_error_equality() {
    let a = ConfigFieldError {
        module_id: "a".to_string(),
        field: "b".to_string(),
        expected: "bool",
        actual: "42".to_string(),
    };
    let b = a.clone();
    assert_eq!(a, b);
}

// ============================================================================
// Typed getters: get_bool
// ============================================================================

fn store_with_lsp_settings() -> ModuleConfigStore {
    let toml_str = r#"
auto_start = true
timeout = 30
name = "rust-analyzer"
count = 5.5
"#;
    let settings: toml::Value = toml::from_str(toml_str).unwrap();
    let mut configs = HashMap::new();
    configs.insert("lsp".to_string(), settings);
    ModuleConfigStore::new(configs)
}

#[test]
fn get_bool_returns_none_for_missing_module() {
    let store = ModuleConfigStore::new(HashMap::new());
    assert_eq!(store.get_bool("lsp", "auto_start").unwrap(), None);
}

#[test]
fn get_bool_returns_none_for_missing_field() {
    let store = store_with_lsp_settings();
    assert_eq!(store.get_bool("lsp", "nonexistent").unwrap(), None);
}

#[test]
fn get_bool_returns_value_for_bool_field() {
    let store = store_with_lsp_settings();
    assert_eq!(store.get_bool("lsp", "auto_start").unwrap(), Some(true));
}

#[test]
fn get_bool_error_for_wrong_type() {
    let store = store_with_lsp_settings();
    let err = store.get_bool("lsp", "timeout").unwrap_err();
    assert_eq!(err.module_id, "lsp");
    assert_eq!(err.field, "timeout");
    assert_eq!(err.expected, "bool");
}

// ============================================================================
// Typed getters: get_int
// ============================================================================

#[test]
fn get_int_returns_none_for_missing_module() {
    let store = ModuleConfigStore::new(HashMap::new());
    assert_eq!(store.get_int("lsp", "timeout").unwrap(), None);
}

#[test]
fn get_int_returns_none_for_missing_field() {
    let store = store_with_lsp_settings();
    assert_eq!(store.get_int("lsp", "nonexistent").unwrap(), None);
}

#[test]
fn get_int_returns_value_for_int_field() {
    let store = store_with_lsp_settings();
    assert_eq!(store.get_int("lsp", "timeout").unwrap(), Some(30));
}

#[test]
fn get_int_error_for_wrong_type() {
    let store = store_with_lsp_settings();
    let err = store.get_int("lsp", "auto_start").unwrap_err();
    assert_eq!(err.module_id, "lsp");
    assert_eq!(err.field, "auto_start");
    assert_eq!(err.expected, "integer");
}

// ============================================================================
// Typed getters: get_str
// ============================================================================

#[test]
fn get_str_returns_none_for_missing_module() {
    let store = ModuleConfigStore::new(HashMap::new());
    assert_eq!(store.get_str("lsp", "name").unwrap(), None);
}

#[test]
fn get_str_returns_none_for_missing_field() {
    let store = store_with_lsp_settings();
    assert_eq!(store.get_str("lsp", "nonexistent").unwrap(), None);
}

#[test]
fn get_str_returns_value_for_str_field() {
    let store = store_with_lsp_settings();
    assert_eq!(store.get_str("lsp", "name").unwrap(), Some("rust-analyzer".to_string()));
}

#[test]
fn get_str_error_for_wrong_type() {
    let store = store_with_lsp_settings();
    let err = store.get_str("lsp", "timeout").unwrap_err();
    assert_eq!(err.module_id, "lsp");
    assert_eq!(err.field, "timeout");
    assert_eq!(err.expected, "string");
}

// ============================================================================
// Edge cases: Unicode values in settings
// ============================================================================

#[test]
fn parse_unicode_string_in_settings() {
    let toml = "
[modules.editor]
enabled = true
[modules.editor.settings]
greeting = \"\u{00e9}\u{00e8}\u{00ea}\u{00eb}\"
locale = \"\u{65e5}\u{672c}\u{8a9e}\"
emoji_name = \"\u{1f680} rocket\"
";
    let config = ModulesConfig::parse(toml).unwrap();
    let settings = config.module_settings("editor").unwrap();
    assert_eq!(
        settings.get("greeting").unwrap().as_str(),
        Some("\u{00e9}\u{00e8}\u{00ea}\u{00eb}")
    );
    assert_eq!(settings.get("locale").unwrap().as_str(), Some("\u{65e5}\u{672c}\u{8a9e}"));
    assert_eq!(settings.get("emoji_name").unwrap().as_str(), Some("\u{1f680} rocket"));
}

#[test]
fn get_str_returns_unicode_value() {
    let toml_str = "
greeting = \"\u{00e9}\u{00e8}\u{00ea}\u{00eb}\"
locale = \"\u{65e5}\u{672c}\u{8a9e}\"
";
    let settings: toml::Value = toml::from_str(toml_str).unwrap();
    let mut configs = HashMap::new();
    configs.insert("i18n".to_string(), settings);
    let store = ModuleConfigStore::new(configs);

    assert_eq!(
        store.get_str("i18n", "greeting").unwrap(),
        Some("\u{00e9}\u{00e8}\u{00ea}\u{00eb}".to_string())
    );
    assert_eq!(
        store.get_str("i18n", "locale").unwrap(),
        Some("\u{65e5}\u{672c}\u{8a9e}".to_string())
    );
}

#[test]
fn unicode_module_id_in_config() {
    let toml = "
[modules.\"\u{00e9}ditor\"]
enabled = false
";
    let config = ModulesConfig::parse(toml).unwrap();
    assert!(!config.is_module_enabled("\u{00e9}ditor"));
    let disabled = config.disabled_modules();
    assert!(disabled.contains(&"\u{00e9}ditor"));
}

#[test]
fn validate_warns_on_unicode_unknown_module() {
    let toml = "
[modules.\"\u{00e9}ditor\"]
enabled = true
";
    let config = ModulesConfig::parse(toml).unwrap();
    let warnings = config.validate_modules(&["vim", "editor"]);
    assert_eq!(warnings.len(), 1);
    assert!(warnings.iter().any(|w| matches!(
        w,
        ModuleConfigWarning::UnknownModule { id } if id == "\u{00e9}ditor"
    )));
}

// ============================================================================
// Edge cases: Very large TOML collections
// ============================================================================

#[test]
fn parse_large_number_of_modules() {
    let mut toml = String::from("extends = \"official\"\n");
    for i in 0..200 {
        let _ = writeln!(toml, "\n[modules.\"module-{i}\"]\nenabled = {}", i % 2 == 0);
    }
    let config = ModulesConfig::parse(&toml).unwrap();
    assert_eq!(config.modules.len(), 200);

    // Verify enabled/disabled pattern
    assert!(config.is_module_enabled("module-0"));
    assert!(!config.is_module_enabled("module-1"));
    assert!(config.is_module_enabled("module-198"));
    assert!(!config.is_module_enabled("module-199"));

    let disabled = config.disabled_modules();
    assert_eq!(disabled.len(), 100);
}

#[test]
fn parse_large_settings_table() {
    let mut toml = String::from("[modules.big]\nenabled = true\n[modules.big.settings]\n");
    for i in 0..500 {
        let _ = writeln!(toml, "key_{i} = {i}");
    }
    let config = ModulesConfig::parse(&toml).unwrap();
    let settings = config.module_settings("big").unwrap();

    // Verify first and last entries
    assert_eq!(settings.get("key_0").unwrap().as_integer(), Some(0));
    assert_eq!(settings.get("key_499").unwrap().as_integer(), Some(499));
}

#[test]
fn build_config_store_large_collection() {
    let mut toml = String::new();
    for i in 0..100 {
        let _ = writeln!(
            toml,
            "[modules.\"mod-{i}\"]\nenabled = true\n[modules.\"mod-{i}\".settings]\nvalue = {i}\n"
        );
    }
    let config = ModulesConfig::parse(&toml).unwrap();
    let store = config.build_config_store();

    // All 100 modules have settings
    assert!(!store.is_empty());
    for i in 0..100 {
        let key = format!("mod-{i}");
        let val = store.get(&key).unwrap();
        assert_eq!(val.get("value").unwrap().as_integer(), Some(i64::from(i)));
    }
}

#[test]
fn validate_large_known_list() {
    let mut toml = String::new();
    for i in 0..50 {
        let _ = writeln!(toml, "[modules.\"mod-{i}\"]\nenabled = true");
    }
    let config = ModulesConfig::parse(&toml).unwrap();

    // Build a known list that covers all but 5 modules
    let known: Vec<String> = (0..45).map(|i| format!("mod-{i}")).collect();
    let known_refs: Vec<&str> = known.iter().map(String::as_str).collect();
    let warnings = config.validate_modules(&known_refs);
    assert_eq!(warnings.len(), 5);
}

#[test]
fn parse_large_extensions_collection() {
    let mut toml = String::new();
    for i in 0..100 {
        let _ = writeln!(toml, "[extensions.\"ext-{i}\"]\nenabled = {}", i % 3 != 0);
    }
    let config = ModulesConfig::parse(&toml).unwrap();
    assert_eq!(config.extensions.len(), 100);

    let disabled = config.disabled_extensions();
    // Every 3rd extension (0, 3, 6, ..., 99) is disabled: ceil(100/3) = 34
    assert_eq!(disabled.len(), 34);
}
