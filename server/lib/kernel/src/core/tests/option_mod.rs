use super::*;

#[test]
fn test_registry_register() {
    let registry = OptionRegistry::new();

    let result = registry.register(OptionSpec::new(
        "number",
        "Show line numbers",
        OptionValue::bool(false),
    ));
    assert!(result.is_ok());
    assert!(registry.contains("number"));
}

#[test]
fn test_registry_register_duplicate() {
    let registry = OptionRegistry::new();

    assert!(
        registry
            .register(OptionSpec::new("number", "desc", OptionValue::bool(false)))
            .is_ok()
    );

    let result = registry.register(OptionSpec::new("number", "desc2", OptionValue::bool(true)));
    assert!(matches!(result, Err(OptionError::AlreadyExists(_))));
}

#[test]
fn test_registry_alias() {
    let registry = OptionRegistry::new();

    assert!(
        registry
            .register(
                OptionSpec::new("number", "Show line numbers", OptionValue::bool(false))
                    .with_short("nu"),
            )
            .is_ok()
    );

    assert!(registry.contains("number"));
    assert!(registry.contains("nu"));
    assert_eq!(registry.resolve_name("nu"), Some("number".to_string()));
}

#[test]
fn test_registry_get_default() {
    let registry = OptionRegistry::new();

    assert!(
        registry
            .register(OptionSpec::new("tabwidth", "Tab width", OptionValue::int(4)))
            .is_ok()
    );

    let value = registry.get("tabwidth", OptionScopeId::Global);
    assert_eq!(value, Some(OptionValue::int(4)));
}

#[test]
fn test_registry_set_and_get() {
    let registry = OptionRegistry::new();

    assert!(
        registry
            .register(OptionSpec::new("tabwidth", "Tab width", OptionValue::int(4)))
            .is_ok()
    );

    assert!(
        registry
            .set("tabwidth", OptionValue::int(8), OptionScopeId::Global)
            .is_ok()
    );

    let value = registry.get("tabwidth", OptionScopeId::Global);
    assert_eq!(value, Some(OptionValue::int(8)));
}

#[test]
fn test_registry_buffer_local() {
    let registry = OptionRegistry::new();

    assert!(
        registry
            .register(
                OptionSpec::new("tabwidth", "Tab width", OptionValue::int(4))
                    .with_scope(OptionScope::Buffer),
            )
            .is_ok()
    );

    let buffer1 = BufferId::new();
    let buffer2 = BufferId::new();

    // Set buffer-local value for buffer1
    assert!(
        registry
            .set_for_buffer("tabwidth", OptionValue::int(2), buffer1)
            .is_ok()
    );

    // buffer1 should have 2, buffer2 should have default (4)
    assert_eq!(registry.get_for_buffer("tabwidth", buffer1), Some(OptionValue::int(2)));
    assert_eq!(registry.get_for_buffer("tabwidth", buffer2), Some(OptionValue::int(4)));
}

#[test]
fn test_registry_window_local() {
    let registry = OptionRegistry::new();

    assert!(
        registry
            .register(
                OptionSpec::new("number", "Show line numbers", OptionValue::bool(false))
                    .with_scope(OptionScope::Window),
            )
            .is_ok()
    );

    let window1 = WindowId::new();
    let window2 = WindowId::new();

    assert!(
        registry
            .set_for_window("number", OptionValue::bool(true), window1)
            .is_ok()
    );

    assert_eq!(registry.get_for_window("number", window1), Some(OptionValue::bool(true)));
    assert_eq!(registry.get_for_window("number", window2), Some(OptionValue::bool(false)));
}

#[test]
fn test_registry_scope_mismatch() {
    let registry = OptionRegistry::new();

    assert!(
        registry
            .register(
                OptionSpec::new("theme", "Color theme", OptionValue::string("default"))
                    .with_scope(OptionScope::Global),
            )
            .is_ok()
    );

    let result = registry.set_for_buffer("theme", OptionValue::string("dark"), BufferId::new());
    assert!(matches!(result, Err(OptionError::ScopeMismatch { .. })));
}

#[test]
fn test_registry_reset() {
    let registry = OptionRegistry::new();

    assert!(
        registry
            .register(OptionSpec::new("tabwidth", "Tab width", OptionValue::int(4)))
            .is_ok()
    );

    assert!(
        registry
            .set("tabwidth", OptionValue::int(8), OptionScopeId::Global)
            .is_ok()
    );
    assert_eq!(registry.get("tabwidth", OptionScopeId::Global), Some(OptionValue::int(8)));

    assert!(registry.reset("tabwidth", OptionScopeId::Global).is_ok());
    assert_eq!(registry.get("tabwidth", OptionScopeId::Global), Some(OptionValue::int(4)));
}

#[test]
fn test_registry_toggle() {
    let registry = OptionRegistry::new();

    assert!(
        registry
            .register(OptionSpec::new("number", "Show line numbers", OptionValue::bool(false)))
            .is_ok()
    );

    let result = registry.toggle("number", OptionScopeId::Global);
    assert!(result.is_ok());
    assert!(result.is_ok_and(|v| v));

    let result = registry.toggle("number", OptionScopeId::Global);
    assert!(result.is_ok());
    assert!(result.is_ok_and(|v| !v));
}

#[test]
fn test_registry_list_matching() {
    let registry = OptionRegistry::new();

    assert!(
        registry
            .register(OptionSpec::new("number", "desc", OptionValue::bool(false)))
            .is_ok()
    );
    assert!(
        registry
            .register(OptionSpec::new("numberwidth", "desc", OptionValue::int(4)))
            .is_ok()
    );
    assert!(
        registry
            .register(OptionSpec::new("wrap", "desc", OptionValue::bool(true)))
            .is_ok()
    );

    let matches = registry.list_matching("num");
    assert_eq!(matches.len(), 2);
}

#[test]
fn test_registry_clear_buffer() {
    let registry = OptionRegistry::new();

    assert!(
        registry
            .register(
                OptionSpec::new("tabwidth", "Tab width", OptionValue::int(4))
                    .with_scope(OptionScope::Buffer),
            )
            .is_ok()
    );

    let buffer = BufferId::new();
    assert!(
        registry
            .set_for_buffer("tabwidth", OptionValue::int(2), buffer)
            .is_ok()
    );

    assert_eq!(registry.get_for_buffer("tabwidth", buffer), Some(OptionValue::int(2)));

    registry.clear_buffer(buffer);

    // Should be back to default
    assert_eq!(registry.get_for_buffer("tabwidth", buffer), Some(OptionValue::int(4)));
}

#[test]
fn test_registry_validation_on_set() {
    let registry = OptionRegistry::new();

    assert!(
        registry
            .register(
                OptionSpec::new("tabwidth", "Tab width", OptionValue::int(4))
                    .with_constraint(OptionConstraint::range(1, 32)),
            )
            .is_ok()
    );

    // Valid value
    assert!(
        registry
            .set("tabwidth", OptionValue::int(8), OptionScopeId::Global)
            .is_ok()
    );

    // Invalid value
    assert!(
        registry
            .set("tabwidth", OptionValue::int(100), OptionScopeId::Global)
            .is_err()
    );
}

// === toggle non-boolean ===

#[test]
fn test_registry_toggle_non_boolean() {
    let registry = OptionRegistry::new();
    assert!(
        registry
            .register(OptionSpec::new("tabwidth", "desc", OptionValue::int(4)))
            .is_ok()
    );
    let result = registry.toggle("tabwidth", OptionScopeId::Global);
    assert!(matches!(result, Err(OptionError::TypeMismatch { .. })));
}

// === list_matching with alias ===

#[test]
fn test_registry_list_matching_alias() {
    let registry = OptionRegistry::new();
    assert!(
        registry
            .register(
                OptionSpec::new("number", "desc", OptionValue::bool(false)).with_short("nu"),
            )
            .is_ok()
    );
    // Prefix "nu" should match via alias
    let matches = registry.list_matching("nu");
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].name, "number");
}

// === list_by_scope ===

#[test]
fn test_registry_list_by_scope() {
    let registry = OptionRegistry::new();
    assert!(
        registry
            .register(
                OptionSpec::new("number", "desc", OptionValue::bool(false))
                    .with_scope(OptionScope::Window),
            )
            .is_ok()
    );
    assert!(
        registry
            .register(
                OptionSpec::new("tabwidth", "desc", OptionValue::int(4))
                    .with_scope(OptionScope::Buffer),
            )
            .is_ok()
    );
    assert!(
        registry
            .register(
                OptionSpec::new("theme", "desc", OptionValue::string("dark"))
                    .with_scope(OptionScope::Global),
            )
            .is_ok()
    );

    let window_opts = registry.list_by_scope(OptionScope::Window);
    assert_eq!(window_opts.len(), 1);
    assert_eq!(window_opts[0].name, "number");

    let buffer_opts = registry.list_by_scope(OptionScope::Buffer);
    assert_eq!(buffer_opts.len(), 1);

    let global_opts = registry.list_by_scope(OptionScope::Global);
    assert_eq!(global_opts.len(), 1);
}

// === clear_window ===

#[test]
fn test_registry_clear_window() {
    let registry = OptionRegistry::new();
    assert!(
        registry
            .register(
                OptionSpec::new("number", "desc", OptionValue::bool(false))
                    .with_scope(OptionScope::Window),
            )
            .is_ok()
    );

    let window = WindowId::new();
    assert!(
        registry
            .set_for_window("number", OptionValue::bool(true), window)
            .is_ok()
    );
    assert_eq!(registry.get_for_window("number", window), Some(OptionValue::bool(true)));

    registry.clear_window(window);
    // Should fall back to default
    assert_eq!(registry.get_for_window("number", window), Some(OptionValue::bool(false)));
}

// === reset buffer/window scopes ===

#[test]
fn test_registry_reset_buffer_scope() {
    let registry = OptionRegistry::new();
    assert!(
        registry
            .register(
                OptionSpec::new("tabwidth", "desc", OptionValue::int(4))
                    .with_scope(OptionScope::Buffer),
            )
            .is_ok()
    );

    let buffer = BufferId::new();
    assert!(
        registry
            .set_for_buffer("tabwidth", OptionValue::int(8), buffer)
            .is_ok()
    );

    let removed = registry.reset("tabwidth", OptionScopeId::Buffer(buffer));
    assert!(removed.is_ok());
    assert_eq!(removed.unwrap(), Some(OptionValue::int(8)));

    // Now back to default
    assert_eq!(registry.get_for_buffer("tabwidth", buffer), Some(OptionValue::int(4)));
}

#[test]
fn test_registry_reset_window_scope() {
    let registry = OptionRegistry::new();
    assert!(
        registry
            .register(
                OptionSpec::new("number", "desc", OptionValue::bool(false))
                    .with_scope(OptionScope::Window),
            )
            .is_ok()
    );

    let window = WindowId::new();
    assert!(
        registry
            .set_for_window("number", OptionValue::bool(true), window)
            .is_ok()
    );

    let removed = registry.reset("number", OptionScopeId::Window(window));
    assert!(removed.is_ok());
    assert_eq!(removed.unwrap(), Some(OptionValue::bool(true)));
}

// === alias conflicts ===

#[test]
fn test_registry_alias_conflicts_with_name() {
    let registry = OptionRegistry::new();
    // Register "nu" as a full option name
    assert!(
        registry
            .register(OptionSpec::new("nu", "desc", OptionValue::bool(false)))
            .is_ok()
    );
    // Now try to register "number" with short "nu" - should conflict
    let result = registry
        .register(OptionSpec::new("number", "desc", OptionValue::bool(false)).with_short("nu"));
    assert!(matches!(result, Err(OptionError::AliasConflict(_))));
}

#[test]
fn test_registry_alias_conflicts_with_alias() {
    let registry = OptionRegistry::new();
    assert!(
        registry
            .register(
                OptionSpec::new("number", "desc", OptionValue::bool(false)).with_short("nu"),
            )
            .is_ok()
    );
    // Another option with the same alias should conflict
    let result = registry.register(
        OptionSpec::new("numbers", "desc", OptionValue::bool(false)).with_short("nu"),
    );
    assert!(matches!(result, Err(OptionError::AliasConflict(_))));
}

// === scope mismatch window on global ===

#[test]
fn test_registry_scope_mismatch_window_on_global() {
    let registry = OptionRegistry::new();
    assert!(
        registry
            .register(
                OptionSpec::new("theme", "desc", OptionValue::string("dark"))
                    .with_scope(OptionScope::Global),
            )
            .is_ok()
    );

    let result =
        registry.set_for_window("theme", OptionValue::string("light"), WindowId::new());
    assert!(matches!(result, Err(OptionError::ScopeMismatch { .. })));
}

// === Coverage: get_spec via alias ===

#[test]
fn test_registry_get_spec_via_alias() {
    let registry = OptionRegistry::new();
    assert!(
        registry
            .register(
                OptionSpec::new("number", "desc", OptionValue::bool(false)).with_short("nu"),
            )
            .is_ok()
    );

    let spec = registry.get_spec("nu");
    assert!(spec.is_some());
    assert_eq!(spec.unwrap().name, "number");

    // Also get_spec by full name
    let spec = registry.get_spec("number");
    assert!(spec.is_some());
}

#[test]
fn test_registry_get_spec_nonexistent() {
    let registry = OptionRegistry::new();
    assert!(registry.get_spec("nope").is_none());
}

// === Coverage: get_global convenience ===

#[test]
fn test_registry_get_global_convenience() {
    let registry = OptionRegistry::new();
    assert!(
        registry
            .register(OptionSpec::new("tabwidth", "desc", OptionValue::int(4)))
            .is_ok()
    );

    let val = registry.get_global("tabwidth");
    assert_eq!(val, Some(OptionValue::int(4)));

    let val = registry.get_global("nonexistent");
    assert_eq!(val, None);
}

// === Coverage: set_global convenience ===

#[test]
fn test_registry_set_global_convenience() {
    let registry = OptionRegistry::new();
    assert!(
        registry
            .register(OptionSpec::new("tabwidth", "desc", OptionValue::int(4)))
            .is_ok()
    );

    assert!(registry.set_global("tabwidth", OptionValue::int(8)).is_ok());
    assert_eq!(registry.get_global("tabwidth"), Some(OptionValue::int(8)));
}

// === Coverage: set not found ===

#[test]
fn test_registry_set_not_found() {
    let registry = OptionRegistry::new();
    let result = registry.set("nonexistent", OptionValue::int(1), OptionScopeId::Global);
    assert!(matches!(result, Err(OptionError::NotFound(_))));
}

// === Coverage: toggle not found ===

#[test]
fn test_registry_toggle_not_found() {
    let registry = OptionRegistry::new();
    let result = registry.toggle("nonexistent", OptionScopeId::Global);
    assert!(matches!(result, Err(OptionError::NotFound(_))));
}

// === Coverage: reset not found ===

#[test]
fn test_registry_reset_not_found() {
    let registry = OptionRegistry::new();
    let result = registry.reset("nonexistent", OptionScopeId::Global);
    assert!(matches!(result, Err(OptionError::NotFound(_))));
}

// === Coverage: list_matching via alias prefix ===

#[test]
fn test_registry_list_matching_alias_prefix() {
    let registry = OptionRegistry::new();
    assert!(
        registry
            .register(
                OptionSpec::new("number", "desc", OptionValue::bool(false)).with_short("nu"),
            )
            .is_ok()
    );
    assert!(
        registry
            .register(OptionSpec::new("numberwidth", "desc", OptionValue::int(4)))
            .is_ok()
    );

    // "nu" prefix matches alias AND "number" and "numberwidth"
    let matches = registry.list_matching("nu");
    assert!(matches.len() >= 2);
}

// === Coverage: len and is_empty ===

#[test]
fn test_registry_len_and_is_empty() {
    let registry = OptionRegistry::new();
    assert!(registry.is_empty());
    assert_eq!(registry.len(), 0);

    assert!(
        registry
            .register(OptionSpec::new("number", "desc", OptionValue::bool(false)))
            .is_ok()
    );
    assert!(!registry.is_empty());
    assert_eq!(registry.len(), 1);
}

// === Coverage: set via alias ===

#[test]
fn test_registry_set_via_alias() {
    let registry = OptionRegistry::new();
    assert!(
        registry
            .register(
                OptionSpec::new("number", "desc", OptionValue::bool(false)).with_short("nu"),
            )
            .is_ok()
    );

    assert!(
        registry
            .set("nu", OptionValue::bool(true), OptionScopeId::Global)
            .is_ok()
    );
    assert_eq!(registry.get("number", OptionScopeId::Global), Some(OptionValue::bool(true)));
}

// === Coverage: list_all ===

#[test]
fn test_registry_list_all() {
    let registry = OptionRegistry::new();
    assert!(
        registry
            .register(OptionSpec::new("a", "desc", OptionValue::bool(false)))
            .is_ok()
    );
    assert!(
        registry
            .register(OptionSpec::new("b", "desc", OptionValue::int(1)))
            .is_ok()
    );

    let all = registry.list_all();
    assert_eq!(all.len(), 2);
    assert!(all.contains(&"a".to_string()));
    assert!(all.contains(&"b".to_string()));
}

// === MC/DC: register() - AlreadyExists branch (line ~128-129) ===
// Exercises the duplicate name check returning AlreadyExists error.

#[test]
fn test_registry_register_already_exists_error() {
    let registry = OptionRegistry::new();
    let spec = OptionSpec::new("tabstop", "Tab stop width", OptionValue::int(8));
    assert!(registry.register(spec).is_ok());

    // Second registration of the same name must return AlreadyExists
    let dup = OptionSpec::new("tabstop", "Duplicate", OptionValue::int(4));
    let result = registry.register(dup);
    assert!(matches!(result, Err(OptionError::AlreadyExists(name)) if name == "tabstop"));
}

// === MC/DC: register() - AliasConflict when alias matches existing spec name ===
// Exercises the check at line ~137: alias == an existing full spec name.

#[test]
fn test_registry_register_alias_conflicts_with_existing_spec_name() {
    let registry = OptionRegistry::new();
    // Register "ts" as a full spec name
    assert!(
        registry
            .register(OptionSpec::new("ts", "Existing spec", OptionValue::int(4)))
            .is_ok()
    );
    // Try to register "tabstop" with short alias "ts" - should fail because "ts" is a spec name
    let result = registry
        .register(OptionSpec::new("tabstop", "desc", OptionValue::int(8)).with_short("ts"));
    assert!(matches!(result, Err(OptionError::AliasConflict(alias)) if alias == "ts"));
}

// === MC/DC: register() - AliasConflict when alias matches existing alias ===
// Exercises the check at line ~142: alias already registered as an alias.

#[test]
fn test_registry_register_alias_conflicts_with_existing_alias() {
    let registry = OptionRegistry::new();
    // Register "number" with alias "nu"
    assert!(
        registry
            .register(
                OptionSpec::new("number", "Line numbers", OptionValue::bool(false))
                    .with_short("nu"),
            )
            .is_ok()
    );
    // Try to register "numberwidth" with the same alias "nu"
    let result = registry.register(
        OptionSpec::new("numberwidth", "Number column width", OptionValue::int(4))
            .with_short("nu"),
    );
    assert!(matches!(result, Err(OptionError::AliasConflict(alias)) if alias == "nu"));
}

// === MC/DC: resolve_name() - TRUE branch: name is a direct spec name (line ~163) ===
// Exercises `if self.specs.read().contains_key(name)` returning true.

#[test]
fn test_resolve_name_returns_direct_spec_name() {
    let registry = OptionRegistry::new();
    assert!(
        registry
            .register(OptionSpec::new("wrap", "Line wrap", OptionValue::bool(true)))
            .is_ok()
    );

    // "wrap" is a registered spec name - resolve_name should return it directly
    let resolved = registry.resolve_name("wrap");
    assert_eq!(resolved, Some("wrap".to_string()));
}

// === MC/DC: get() - Window scope value found (line ~207) ===
// Exercises `if let Some(value) = self.window_values.read().get(...)` TRUE branch.

#[test]
fn test_get_returns_window_local_value_when_set() {
    let registry = OptionRegistry::new();
    assert!(
        registry
            .register(
                OptionSpec::new("number", "Line numbers", OptionValue::bool(false))
                    .with_scope(OptionScope::Window),
            )
            .is_ok()
    );

    let window_id = WindowId::new();
    // Set a window-local value
    assert!(
        registry
            .set_for_window("number", OptionValue::bool(true), window_id)
            .is_ok()
    );

    // get() with Window scope must return the window-local value (TRUE branch at ~207)
    let value = registry.get("number", OptionScopeId::Window(window_id));
    assert_eq!(value, Some(OptionValue::bool(true)));
}

// === MC/DC: get() - Buffer scope value found (line ~217) ===
// Exercises `if let Some(value) = self.buffer_values.read().get(...)` TRUE branch.

#[test]
fn test_get_returns_buffer_local_value_when_set() {
    let registry = OptionRegistry::new();
    assert!(
        registry
            .register(
                OptionSpec::new("tabstop", "Tab width", OptionValue::int(4))
                    .with_scope(OptionScope::Buffer),
            )
            .is_ok()
    );

    let buffer_id = BufferId::new();
    // Set a buffer-local value
    assert!(
        registry
            .set_for_buffer("tabstop", OptionValue::int(2), buffer_id)
            .is_ok()
    );

    // get() with Buffer scope must return the buffer-local value (TRUE branch at ~217)
    let value = registry.get("tabstop", OptionScopeId::Buffer(buffer_id));
    assert_eq!(value, Some(OptionValue::int(2)));
}

// === MC/DC: get() - global override value present (line ~229) ===
// Exercises `if let Some(value) = self.global_values.read().get(&full_name)` TRUE branch.

#[test]
fn test_get_returns_global_override_when_set() {
    let registry = OptionRegistry::new();
    assert!(
        registry
            .register(OptionSpec::new("tabstop", "Tab width", OptionValue::int(4)))
            .is_ok()
    );

    // Set a global override (different from default)
    assert!(registry.set_global("tabstop", OptionValue::int(8)).is_ok());

    // get() with Global scope must return the global override (TRUE branch at ~229)
    let value = registry.get("tabstop", OptionScopeId::Global);
    assert_eq!(value, Some(OptionValue::int(8)));
}
