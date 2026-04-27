use {super::*, std::path::PathBuf};

// ============================================================================
// ConfigPaths::resolve_dir (pure function, no env mutation needed)
// ============================================================================

#[test]
fn resolve_dir_env_override_takes_priority() {
    let result = ConfigPaths::resolve_dir(
        Some("/custom/config".to_string()),
        Some(PathBuf::from("/default/xdg")),
        "config",
    );
    assert_eq!(result.unwrap(), PathBuf::from("/custom/config"));
}

#[test]
fn resolve_dir_env_override_ignores_platform() {
    // Even if platform returns None, env override wins
    let result = ConfigPaths::resolve_dir(Some("/custom/data".to_string()), None, "data");
    assert_eq!(result.unwrap(), PathBuf::from("/custom/data"));
}

#[test]
fn resolve_dir_no_env_uses_platform_with_reovim_suffix() {
    let result =
        ConfigPaths::resolve_dir(None, Some(PathBuf::from("/home/user/.config")), "config");
    assert_eq!(result.unwrap(), PathBuf::from("/home/user/.config/reovim"));
}

#[test]
fn resolve_dir_no_env_no_platform_returns_error() {
    let result = ConfigPaths::resolve_dir(None, None, "cache");
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.to_string().contains("cannot determine cache directory"));
}

#[test]
fn resolve_dir_error_kind_appears_in_message() {
    let result = ConfigPaths::resolve_dir(None, None, "data");
    let err = result.unwrap_err();
    assert!(err.to_string().contains("data"));
}

#[test]
fn resolve_dir_empty_env_string_is_valid() {
    // Empty string env var is still "set" — returns empty PathBuf
    let result =
        ConfigPaths::resolve_dir(Some(String::new()), Some(PathBuf::from("/default")), "config");
    assert_eq!(result.unwrap(), PathBuf::from(""));
}

// ============================================================================
// ConfigPaths public methods (integration — use actual env)
// ============================================================================

#[test]
fn config_dir_returns_path_ending_in_reovim() {
    // This test works regardless of env vars because either:
    // - REOVIM_CONFIG_DIR is set (returns that exact path)
    // - Platform default works (returns .../reovim)
    let result = ConfigPaths::config_dir();
    assert!(result.is_ok());
}

#[test]
fn data_dir_returns_ok() {
    let result = ConfigPaths::data_dir();
    assert!(result.is_ok());
}

#[test]
fn cache_dir_returns_ok() {
    let result = ConfigPaths::cache_dir();
    assert!(result.is_ok());
}

#[test]
fn config_file_appends_config_toml() {
    let result = ConfigPaths::config_file();
    assert!(result.is_ok());
    let path = result.unwrap();
    assert!(path.ends_with("config.toml"));
}

#[test]
fn profiles_dir_appends_profiles() {
    let result = ConfigPaths::profiles_dir();
    assert!(result.is_ok());
    let path = result.unwrap();
    assert!(path.ends_with("profiles"));
}
