use super::*;

// ========================================================================
// ComponentId: constants
// ========================================================================

#[test]
fn test_component_id_editor_constant() {
    assert_eq!(ComponentId::EDITOR.as_str(), "editor");
}

#[test]
fn test_component_id_window_constant() {
    assert_eq!(ComponentId::WINDOW.as_str(), "window");
}

#[test]
fn test_component_id_command_line_constant() {
    assert_eq!(ComponentId::COMMAND_LINE.as_str(), "command_line");
}

#[test]
fn test_component_id_explorer_constant() {
    assert_eq!(ComponentId::EXPLORER.as_str(), "explorer");
}

#[test]
fn test_component_id_telescope_constant() {
    assert_eq!(ComponentId::TELESCOPE.as_str(), "telescope");
}

// ========================================================================
// ComponentId: construction
// ========================================================================

#[test]
fn test_component_id_new() {
    let id = ComponentId::new("my_component");
    assert_eq!(id.as_str(), "my_component");
}

#[test]
fn test_component_id_custom() {
    let custom = ComponentId::custom("my_plugin");
    assert_eq!(custom.as_str(), "my_plugin");
}

#[test]
fn test_component_id_new_and_custom_are_equivalent() {
    let from_new = ComponentId::new("test_id");
    let from_custom = ComponentId::custom("test_id");
    assert_eq!(from_new, from_custom);
    assert_eq!(from_new.as_str(), from_custom.as_str());
}

#[test]
fn test_component_id_new_matches_constant() {
    assert_eq!(ComponentId::EDITOR, ComponentId::new("editor"));
    assert_eq!(ComponentId::WINDOW, ComponentId::new("window"));
    assert_eq!(ComponentId::COMMAND_LINE, ComponentId::new("command_line"));
    assert_eq!(ComponentId::EXPLORER, ComponentId::new("explorer"));
    assert_eq!(ComponentId::TELESCOPE, ComponentId::new("telescope"));
}

#[test]
fn test_component_id_empty_string() {
    let id = ComponentId::new("");
    assert_eq!(id.as_str(), "");
}

// ========================================================================
// ComponentId: equality and inequality
// ========================================================================

#[test]
fn test_component_id_equality_same() {
    assert_eq!(ComponentId::EDITOR, ComponentId::new("editor"));
}

#[test]
fn test_component_id_inequality_different() {
    assert_ne!(ComponentId::EDITOR, ComponentId::WINDOW);
}

#[test]
fn test_component_id_all_constants_distinct() {
    let ids = [
        ComponentId::EDITOR,
        ComponentId::WINDOW,
        ComponentId::COMMAND_LINE,
        ComponentId::EXPLORER,
        ComponentId::TELESCOPE,
    ];
    for i in 0..ids.len() {
        for j in (i + 1)..ids.len() {
            assert_ne!(ids[i], ids[j], "Constants at indices {i} and {j} must be different");
        }
    }
}

// ========================================================================
// ComponentId: trait implementations (Debug, Clone, Copy, Hash)
// ========================================================================

#[test]
fn test_component_id_debug() {
    let id = ComponentId::EDITOR;
    let debug = format!("{id:?}");
    assert!(debug.contains("editor"), "Debug output should contain the id string");
}

#[test]
fn test_component_id_clone() {
    let original = ComponentId::EDITOR;
    let cloned = original;
    assert_eq!(original, cloned);
}

#[test]
fn test_component_id_copy() {
    let original = ComponentId::WINDOW;
    let copied = original;
    // Both should still be valid (Copy semantics)
    assert_eq!(original, copied);
    assert_eq!(original.as_str(), "window");
    assert_eq!(copied.as_str(), "window");
}

#[test]
fn test_component_id_hash_in_map() {
    let mut map = HashMap::new();
    map.insert(ComponentId::EDITOR, "editor_value");
    map.insert(ComponentId::WINDOW, "window_value");

    assert_eq!(map.get(&ComponentId::EDITOR), Some(&"editor_value"));
    assert_eq!(map.get(&ComponentId::WINDOW), Some(&"window_value"));
    assert_eq!(map.get(&ComponentId::custom("missing")), None);
}

#[test]
fn test_component_id_hash_consistency() {
    use std::collections::HashSet;

    let mut set = HashSet::new();
    set.insert(ComponentId::EDITOR);
    set.insert(ComponentId::new("editor")); // Same value

    assert_eq!(set.len(), 1, "Same component id should hash to same bucket");
}

// ========================================================================
// InteractorConfig: constructors
// ========================================================================

#[test]
fn test_interactor_config_accepting_input() {
    let config = InteractorConfig::accepting_input();
    assert!(config.accepts_char_input);
}

#[test]
fn test_interactor_config_using_keymap() {
    let config = InteractorConfig::using_keymap();
    assert!(!config.accepts_char_input);
}

#[test]
fn test_interactor_config_default() {
    let config = InteractorConfig::default();
    assert!(config.accepts_char_input, "Default should accept char input");
}

#[test]
fn test_interactor_config_default_matches_accepting_input() {
    let default = InteractorConfig::default();
    let accepting = InteractorConfig::accepting_input();
    assert_eq!(default.accepts_char_input, accepting.accepts_char_input);
}

// ========================================================================
// InteractorConfig: trait implementations
// ========================================================================

#[test]
fn test_interactor_config_debug() {
    let config = InteractorConfig::accepting_input();
    let debug = format!("{config:?}");
    assert!(debug.contains("InteractorConfig"), "Debug output should contain type name");
    assert!(debug.contains("accepts_char_input"), "Debug output should contain field name");
}

#[test]
fn test_interactor_config_clone() {
    let original = InteractorConfig::using_keymap();
    let cloned = original;
    assert_eq!(original.accepts_char_input, cloned.accepts_char_input);
}

#[test]
fn test_interactor_config_copy() {
    let original = InteractorConfig::accepting_input();
    let copied = original;
    // Both should still be valid (Copy semantics)
    assert!(original.accepts_char_input);
    assert!(copied.accepts_char_input);
}

// ========================================================================
// InteractorConfig: field access
// ========================================================================

#[test]
fn test_interactor_config_field_direct_access() {
    let mut config = InteractorConfig::accepting_input();
    assert!(config.accepts_char_input);

    config.accepts_char_input = false;
    assert!(!config.accepts_char_input);
}

// ========================================================================
// InteractorRegistry: construction
// ========================================================================

#[test]
fn test_registry_new_is_empty() {
    let registry = InteractorRegistry::new();
    assert!(registry.is_empty());
    assert_eq!(registry.len(), 0);
}

#[test]
fn test_registry_default_is_empty() {
    let registry = InteractorRegistry::default();
    assert!(registry.is_empty());
    assert_eq!(registry.len(), 0);
}

#[test]
fn test_registry_new_and_default_equivalent() {
    let r1 = InteractorRegistry::new();
    let r2 = InteractorRegistry::default();
    assert_eq!(r1.len(), r2.len());
    assert_eq!(r1.is_empty(), r2.is_empty());
}

#[test]
fn test_registry_with_builtins_not_empty() {
    let registry = InteractorRegistry::with_builtins();
    assert!(!registry.is_empty());
}

#[test]
fn test_registry_with_builtins_has_window() {
    let registry = InteractorRegistry::with_builtins();
    assert!(registry.get(&ComponentId::WINDOW).is_some());
    assert!(!registry.accepts_char_input(&ComponentId::WINDOW));
}

#[test]
fn test_registry_with_builtins_len() {
    let registry = InteractorRegistry::with_builtins();
    // Currently only WINDOW is registered as builtin
    assert_eq!(registry.len(), 1);
}

// ========================================================================
// InteractorRegistry: register
// ========================================================================

#[test]
fn test_registry_register_single() {
    let mut registry = InteractorRegistry::new();
    registry.register(ComponentId::WINDOW, InteractorConfig::using_keymap());
    assert_eq!(registry.len(), 1);
    assert!(!registry.is_empty());
}

#[test]
fn test_registry_register_multiple() {
    let mut registry = InteractorRegistry::new();
    registry.register(ComponentId::WINDOW, InteractorConfig::using_keymap());
    registry.register(ComponentId::EXPLORER, InteractorConfig::accepting_input());
    registry.register(ComponentId::TELESCOPE, InteractorConfig::accepting_input());
    assert_eq!(registry.len(), 3);
}

#[test]
fn test_registry_register_replaces_existing() {
    let mut registry = InteractorRegistry::new();

    // Register as using_keymap
    registry.register(ComponentId::WINDOW, InteractorConfig::using_keymap());
    assert!(!registry.accepts_char_input(&ComponentId::WINDOW));

    // Replace with accepting_input
    registry.register(ComponentId::WINDOW, InteractorConfig::accepting_input());
    assert!(registry.accepts_char_input(&ComponentId::WINDOW));

    // Length should still be 1 (replaced, not added)
    assert_eq!(registry.len(), 1);
}

#[test]
fn test_registry_register_custom_component() {
    let mut registry = InteractorRegistry::new();
    let custom = ComponentId::custom("my_plugin_panel");
    registry.register(custom, InteractorConfig::using_keymap());
    assert!(registry.uses_keymap(&custom));
}

// ========================================================================
// InteractorRegistry: register_builtins
// ========================================================================

#[test]
fn test_registry_register_builtins() {
    let mut registry = InteractorRegistry::new();
    assert!(registry.is_empty());

    registry.register_builtins();
    assert!(!registry.is_empty());
    assert!(!registry.accepts_char_input(&ComponentId::WINDOW));
}

#[test]
fn test_registry_register_builtins_idempotent() {
    let mut registry = InteractorRegistry::new();
    registry.register_builtins();
    let len_after_first = registry.len();

    registry.register_builtins();
    let len_after_second = registry.len();

    // Calling twice should not change the count (HashMap::insert replaces)
    assert_eq!(len_after_first, len_after_second);
}

// ========================================================================
// InteractorRegistry: unregister
// ========================================================================

#[test]
fn test_registry_unregister_existing() {
    let mut registry = InteractorRegistry::with_builtins();
    let initial_len = registry.len();

    registry.unregister(&ComponentId::WINDOW);
    assert_eq!(registry.len(), initial_len - 1);
    assert!(registry.get(&ComponentId::WINDOW).is_none());
}

#[test]
fn test_registry_unregister_nonexistent_is_noop() {
    let mut registry = InteractorRegistry::new();
    let unknown = ComponentId::custom("nonexistent");

    // Should not panic
    registry.unregister(&unknown);
    assert!(registry.is_empty());
}

#[test]
fn test_registry_unregister_restores_default_behavior() {
    let mut registry = InteractorRegistry::with_builtins();

    // WINDOW uses keymap (registered)
    assert!(registry.uses_keymap(&ComponentId::WINDOW));

    registry.unregister(&ComponentId::WINDOW);

    // After unregister, defaults to accepting char input
    assert!(registry.accepts_char_input(&ComponentId::WINDOW));
    assert!(!registry.uses_keymap(&ComponentId::WINDOW));
}

#[test]
fn test_registry_unregister_then_reregister() {
    let mut registry = InteractorRegistry::new();

    registry.register(ComponentId::EXPLORER, InteractorConfig::accepting_input());
    assert!(registry.accepts_char_input(&ComponentId::EXPLORER));

    registry.unregister(&ComponentId::EXPLORER);
    assert!(registry.is_empty());

    // Re-register with different config
    registry.register(ComponentId::EXPLORER, InteractorConfig::using_keymap());
    assert!(registry.uses_keymap(&ComponentId::EXPLORER));
    assert_eq!(registry.len(), 1);
}

// ========================================================================
// InteractorRegistry: get
// ========================================================================

#[test]
fn test_registry_get_registered() {
    let mut registry = InteractorRegistry::new();
    registry.register(ComponentId::WINDOW, InteractorConfig::using_keymap());

    let config = registry.get(&ComponentId::WINDOW);
    assert!(config.is_some());
    assert!(!config.expect("should be registered").accepts_char_input);
}

#[test]
fn test_registry_get_unregistered() {
    let registry = InteractorRegistry::new();
    assert!(registry.get(&ComponentId::EDITOR).is_none());
}

#[test]
fn test_registry_get_after_unregister() {
    let mut registry = InteractorRegistry::with_builtins();
    assert!(registry.get(&ComponentId::WINDOW).is_some());

    registry.unregister(&ComponentId::WINDOW);
    assert!(registry.get(&ComponentId::WINDOW).is_none());
}

// ========================================================================
// InteractorRegistry: accepts_char_input
// ========================================================================

#[test]
fn test_registry_accepts_char_input_unregistered_defaults_true() {
    let registry = InteractorRegistry::new();
    let unknown = ComponentId::custom("unknown");
    assert!(
        registry.accepts_char_input(&unknown),
        "Unregistered components should default to accepting char input"
    );
}

#[test]
fn test_registry_accepts_char_input_registered_accepting() {
    let mut registry = InteractorRegistry::new();
    registry.register(ComponentId::EXPLORER, InteractorConfig::accepting_input());
    assert!(registry.accepts_char_input(&ComponentId::EXPLORER));
}

#[test]
fn test_registry_accepts_char_input_registered_keymap() {
    let mut registry = InteractorRegistry::new();
    registry.register(ComponentId::WINDOW, InteractorConfig::using_keymap());
    assert!(!registry.accepts_char_input(&ComponentId::WINDOW));
}

#[test]
fn test_registry_accepts_char_input_all_builtin_constants() {
    let registry = InteractorRegistry::new();
    // All built-in constants are unregistered in empty registry; default is true
    assert!(registry.accepts_char_input(&ComponentId::EDITOR));
    assert!(registry.accepts_char_input(&ComponentId::WINDOW));
    assert!(registry.accepts_char_input(&ComponentId::COMMAND_LINE));
    assert!(registry.accepts_char_input(&ComponentId::EXPLORER));
    assert!(registry.accepts_char_input(&ComponentId::TELESCOPE));
}

// ========================================================================
// InteractorRegistry: uses_keymap
// ========================================================================

#[test]
fn test_registry_uses_keymap_unregistered_defaults_false() {
    let registry = InteractorRegistry::new();
    let unknown = ComponentId::custom("unknown");
    assert!(
        !registry.uses_keymap(&unknown),
        "Unregistered components should default to NOT using keymap"
    );
}

#[test]
fn test_registry_uses_keymap_registered_keymap() {
    let mut registry = InteractorRegistry::new();
    registry.register(ComponentId::WINDOW, InteractorConfig::using_keymap());
    assert!(registry.uses_keymap(&ComponentId::WINDOW));
}

#[test]
fn test_registry_uses_keymap_registered_accepting() {
    let mut registry = InteractorRegistry::new();
    registry.register(ComponentId::EXPLORER, InteractorConfig::accepting_input());
    assert!(!registry.uses_keymap(&ComponentId::EXPLORER));
}

#[test]
fn test_registry_uses_keymap_is_inverse_of_accepts_char_input() {
    let mut registry = InteractorRegistry::new();
    registry.register(ComponentId::WINDOW, InteractorConfig::using_keymap());
    registry.register(ComponentId::EXPLORER, InteractorConfig::accepting_input());

    // For registered components
    assert_ne!(
        registry.accepts_char_input(&ComponentId::WINDOW),
        registry.uses_keymap(&ComponentId::WINDOW),
        "uses_keymap should be inverse of accepts_char_input"
    );
    assert_ne!(
        registry.accepts_char_input(&ComponentId::EXPLORER),
        registry.uses_keymap(&ComponentId::EXPLORER),
        "uses_keymap should be inverse of accepts_char_input"
    );

    // For unregistered components
    let unknown = ComponentId::custom("unknown");
    assert_ne!(
        registry.accepts_char_input(&unknown),
        registry.uses_keymap(&unknown),
        "uses_keymap should be inverse of accepts_char_input for unregistered"
    );
}

// ========================================================================
// InteractorRegistry: len and is_empty
// ========================================================================

#[test]
fn test_registry_len_empty() {
    let registry = InteractorRegistry::new();
    assert_eq!(registry.len(), 0);
}

#[test]
fn test_registry_len_after_register() {
    let mut registry = InteractorRegistry::new();
    registry.register(ComponentId::WINDOW, InteractorConfig::using_keymap());
    assert_eq!(registry.len(), 1);

    registry.register(ComponentId::EXPLORER, InteractorConfig::accepting_input());
    assert_eq!(registry.len(), 2);
}

#[test]
fn test_registry_len_after_unregister() {
    let mut registry = InteractorRegistry::new();
    registry.register(ComponentId::WINDOW, InteractorConfig::using_keymap());
    registry.register(ComponentId::EXPLORER, InteractorConfig::accepting_input());
    assert_eq!(registry.len(), 2);

    registry.unregister(&ComponentId::WINDOW);
    assert_eq!(registry.len(), 1);

    registry.unregister(&ComponentId::EXPLORER);
    assert_eq!(registry.len(), 0);
}

#[test]
fn test_registry_is_empty_true() {
    let registry = InteractorRegistry::new();
    assert!(registry.is_empty());
}

#[test]
fn test_registry_is_empty_false() {
    let mut registry = InteractorRegistry::new();
    registry.register(ComponentId::WINDOW, InteractorConfig::using_keymap());
    assert!(!registry.is_empty());
}

#[test]
fn test_registry_is_empty_after_all_removed() {
    let mut registry = InteractorRegistry::new();
    registry.register(ComponentId::WINDOW, InteractorConfig::using_keymap());
    registry.unregister(&ComponentId::WINDOW);
    assert!(registry.is_empty());
}

// ========================================================================
// InteractorRegistry: Debug
// ========================================================================

#[test]
fn test_registry_debug_empty() {
    let registry = InteractorRegistry::new();
    let debug = format!("{registry:?}");
    assert!(
        debug.contains("InteractorRegistry"),
        "Debug output should contain the type name"
    );
}

#[test]
fn test_registry_debug_with_entries() {
    let registry = InteractorRegistry::with_builtins();
    let debug = format!("{registry:?}");
    assert!(debug.contains("InteractorRegistry"));
}

// ========================================================================
// Integration scenarios
// ========================================================================

#[test]
fn test_typical_setup_scenario() {
    // Simulates typical module initialization:
    // 1. Create registry with builtins
    // 2. Add custom interactors from plugins
    // 3. Query behavior at runtime
    let mut registry = InteractorRegistry::with_builtins();

    // Window mode uses keymap (from builtins)
    assert!(registry.uses_keymap(&ComponentId::WINDOW));

    // Explorer plugin registers as accepting input
    registry.register(ComponentId::EXPLORER, InteractorConfig::accepting_input());
    assert!(registry.accepts_char_input(&ComponentId::EXPLORER));

    // Telescope plugin registers as accepting input
    registry.register(ComponentId::TELESCOPE, InteractorConfig::accepting_input());
    assert!(registry.accepts_char_input(&ComponentId::TELESCOPE));

    // Custom plugin registers with keymap
    let panel = ComponentId::custom("file_tree");
    registry.register(panel, InteractorConfig::using_keymap());
    assert!(registry.uses_keymap(&panel));

    // Editor is unregistered, defaults to accepting input
    assert!(registry.accepts_char_input(&ComponentId::EDITOR));

    assert_eq!(registry.len(), 4);
}

#[test]
fn test_mode_transition_scenario() {
    // Simulates a component switching between input modes
    let mut registry = InteractorRegistry::new();

    let explorer = ComponentId::EXPLORER;

    // Explorer initially in navigation mode (keymap)
    registry.register(explorer, InteractorConfig::using_keymap());
    assert!(registry.uses_keymap(&explorer));

    // Explorer switches to filter mode (text input)
    registry.register(explorer, InteractorConfig::accepting_input());
    assert!(registry.accepts_char_input(&explorer));

    // Explorer switches back to navigation mode
    registry.register(explorer, InteractorConfig::using_keymap());
    assert!(registry.uses_keymap(&explorer));
}

#[test]
fn test_plugin_lifecycle_scenario() {
    // Simulates plugin load/unload cycle
    let mut registry = InteractorRegistry::with_builtins();

    let plugin_id = ComponentId::custom("my_plugin");

    // Plugin loads and registers
    registry.register(plugin_id, InteractorConfig::accepting_input());
    assert!(registry.accepts_char_input(&plugin_id));

    // Plugin unloads
    registry.unregister(&plugin_id);
    assert!(registry.get(&plugin_id).is_none());

    // After unload, defaults to accepting (safe default)
    assert!(registry.accepts_char_input(&plugin_id));
}

#[test]
fn test_many_custom_components() {
    let mut registry = InteractorRegistry::new();

    for i in 0..100 {
        // Use leaked string to get 'static lifetime for testing
        let name: &'static str = Box::leak(format!("component_{i}").into_boxed_str());
        let id = ComponentId::new(name);
        if i % 2 == 0 {
            registry.register(id, InteractorConfig::accepting_input());
        } else {
            registry.register(id, InteractorConfig::using_keymap());
        }
    }

    assert_eq!(registry.len(), 100);
}
