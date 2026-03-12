use {
    super::*,
    reovim_kernel::api::v1::{OptionSpec, OptionValue},
};

// ========================================================================
// Profile::new()
// ========================================================================

#[test]
fn test_profile_new() {
    let profile = Profile::new();
    assert_eq!(profile.metadata.version, 1);
    assert!(profile.options.is_empty());
}

#[test]
fn test_profile_default() {
    let profile = Profile::default();
    assert_eq!(profile.metadata.version, 1);
    assert!(profile.options.is_empty());
}

// ========================================================================
// ProfileOption conversions
// ========================================================================

#[test]
fn test_profile_option_bool_roundtrip() {
    let opt = ProfileOption::from_option_value(&OptionValue::bool(true));
    assert_eq!(opt, ProfileOption::Bool { value: true });
    assert_eq!(opt.to_option_value(), OptionValue::bool(true));
}

#[test]
fn test_profile_option_integer_roundtrip() {
    let opt = ProfileOption::from_option_value(&OptionValue::int(42));
    assert_eq!(opt, ProfileOption::Integer { value: 42 });
    assert_eq!(opt.to_option_value(), OptionValue::int(42));
}

#[test]
fn test_profile_option_string_roundtrip() {
    let opt = ProfileOption::from_option_value(&OptionValue::string("gruvbox"));
    assert_eq!(
        opt,
        ProfileOption::Str {
            value: "gruvbox".to_string()
        }
    );
    assert_eq!(opt.to_option_value(), OptionValue::string("gruvbox"));
}

#[test]
fn test_profile_option_choice_roundtrip() {
    let choices = vec!["none".to_string(), "block".to_string(), "all".to_string()];
    let val = OptionValue::choice("block", choices.clone());
    let opt = ProfileOption::from_option_value(&val);
    assert_eq!(
        opt,
        ProfileOption::Choice {
            value: "block".to_string(),
            choices,
        }
    );
    assert_eq!(opt.to_option_value(), val);
}

// ========================================================================
// TOML serialization/deserialization
// ========================================================================

#[test]
fn test_toml_roundtrip_empty() {
    let profile = Profile::new();
    let toml_str = profile.to_toml().unwrap();
    let restored = Profile::from_toml(&toml_str).unwrap();
    assert_eq!(profile, restored);
}

#[test]
fn test_toml_roundtrip_all_types() {
    let mut profile = Profile::new();
    profile
        .options
        .insert("number".to_string(), ProfileOption::Bool { value: true });
    profile
        .options
        .insert("tabwidth".to_string(), ProfileOption::Integer { value: 4 });
    profile.options.insert(
        "theme".to_string(),
        ProfileOption::Str {
            value: "gruvbox".to_string(),
        },
    );
    profile.options.insert(
        "virtualedit".to_string(),
        ProfileOption::Choice {
            value: "block".to_string(),
            choices: vec!["none".to_string(), "block".to_string(), "all".to_string()],
        },
    );

    let toml_str = profile.to_toml().unwrap();
    let restored = Profile::from_toml(&toml_str).unwrap();
    assert_eq!(profile, restored);
}

#[test]
fn test_from_toml_invalid() {
    let err = Profile::from_toml("not valid toml {{{").unwrap_err();
    assert!(err.contains("TOML parse failed"));
}

#[test]
fn test_from_toml_wrong_version() {
    let toml_str = "[metadata]\nversion = 99\n\n[options]\n";
    let err = Profile::from_toml(toml_str).unwrap_err();
    assert!(err.contains("unsupported profile version 99"));
}

// ========================================================================
// Registry snapshot/restore
// ========================================================================

fn test_registry() -> OptionRegistry {
    let registry = OptionRegistry::new();
    registry
        .register(OptionSpec::new("number", "Show line numbers", OptionValue::bool(false)))
        .unwrap();
    registry
        .register(OptionSpec::new("tabwidth", "Tab width", OptionValue::int(8)))
        .unwrap();
    registry
        .register(OptionSpec::new("theme", "Color theme", OptionValue::string("default")))
        .unwrap();
    registry
}

#[test]
fn test_from_option_registry_no_overrides() {
    let registry = test_registry();
    let profile = Profile::from_option_registry(&registry);
    assert!(profile.options.is_empty());
}

#[test]
fn test_from_option_registry_with_overrides() {
    let registry = test_registry();
    registry
        .set_global("number", OptionValue::bool(true))
        .unwrap();
    registry
        .set_global("tabwidth", OptionValue::int(4))
        .unwrap();

    let profile = Profile::from_option_registry(&registry);
    assert_eq!(profile.options.len(), 2);
    assert_eq!(profile.options["number"], ProfileOption::Bool { value: true });
    assert_eq!(profile.options["tabwidth"], ProfileOption::Integer { value: 4 });
}

#[test]
fn test_apply_to_registry() {
    let registry = test_registry();

    let mut profile = Profile::new();
    profile
        .options
        .insert("number".to_string(), ProfileOption::Bool { value: true });
    profile
        .options
        .insert("tabwidth".to_string(), ProfileOption::Integer { value: 2 });

    let warnings = profile.apply_to_registry(&registry);
    assert!(warnings.is_empty());

    assert_eq!(registry.get_global("number"), Some(OptionValue::bool(true)));
    assert_eq!(registry.get_global("tabwidth"), Some(OptionValue::int(2)));
}

#[test]
fn test_apply_to_registry_unknown_option() {
    let registry = test_registry();

    let mut profile = Profile::new();
    profile
        .options
        .insert("nonexistent".to_string(), ProfileOption::Bool { value: true });

    let warnings = profile.apply_to_registry(&registry);
    assert_eq!(warnings.len(), 1);
    assert!(warnings[0].contains("unknown option"));
}

#[test]
fn test_apply_to_registry_type_mismatch() {
    let registry = test_registry();

    let mut profile = Profile::new();
    // number is bool, but we give it an integer
    profile
        .options
        .insert("number".to_string(), ProfileOption::Integer { value: 42 });

    let warnings = profile.apply_to_registry(&registry);
    assert_eq!(warnings.len(), 1);
    assert!(warnings[0].contains("failed to set"));
}

#[test]
fn test_apply_to_registry_mixed_valid_and_invalid() {
    let registry = test_registry();

    let mut profile = Profile::new();
    profile
        .options
        .insert("number".to_string(), ProfileOption::Bool { value: true });
    profile
        .options
        .insert("nonexistent".to_string(), ProfileOption::Bool { value: true });
    // Type mismatch: tabwidth expects integer
    profile
        .options
        .insert("tabwidth".to_string(), ProfileOption::Bool { value: false });

    let warnings = profile.apply_to_registry(&registry);
    assert_eq!(warnings.len(), 2);

    // Valid option should still be applied
    assert_eq!(registry.get_global("number"), Some(OptionValue::bool(true)));
}

#[test]
fn test_full_save_load_roundtrip() {
    let registry = test_registry();
    registry
        .set_global("number", OptionValue::bool(true))
        .unwrap();
    registry
        .set_global("tabwidth", OptionValue::int(4))
        .unwrap();

    let profile = Profile::from_option_registry(&registry);
    let toml_str = profile.to_toml().unwrap();
    let restored = Profile::from_toml(&toml_str).unwrap();

    // Apply to a fresh registry
    let registry2 = test_registry();
    let warnings = restored.apply_to_registry(&registry2);
    assert!(warnings.is_empty());
    assert_eq!(registry2.get_global("number"), Some(OptionValue::bool(true)));
    assert_eq!(registry2.get_global("tabwidth"), Some(OptionValue::int(4)));
}
