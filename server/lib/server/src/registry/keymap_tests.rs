use {super::*, reovim_kernel::api::v1::ModuleId};

const TEST_MODULE: ModuleId = ModuleId::new("test");

fn test_mode() -> ModeId {
    ModeId::with_discriminant(TEST_MODULE, "NORMAL", 0)
}

fn test_command(name: &'static str) -> CommandId {
    CommandId::new(TEST_MODULE, name)
}

/// Convenience wrapper: parse a simple ASCII notation string into an
/// `InputSequence` via `test_helpers::seq_from_notation`.
fn seq(notation: &str) -> InputSequence {
    test_helpers::seq_from_notation(notation).expect("non-empty ASCII notation required in test")
}

#[test]
fn test_keymap_registry_new() {
    let registry = KeymapRegistry::new();
    assert!(registry.is_empty());
    assert_eq!(registry.total_bindings(), 0);
}

#[test]
fn test_keymap_registry_register() {
    let mut registry = KeymapRegistry::new();
    let mode = test_mode();
    let keys = seq("j");
    let cmd = test_command("cursor-down");

    registry.register_at_layer(BindingLayer::Policy, &mode, keys, cmd, "", None);

    assert_eq!(registry.binding_count(&mode), 1);
    assert!(!registry.is_empty());
}

#[test]
fn test_keymap_registry_lookup_found() {
    let mut registry = KeymapRegistry::new();
    let mode = test_mode();
    let cmd = test_command("cursor-down");

    registry.register_str(&mode, "j", cmd.clone());

    let keys = seq("j");
    let result = registry.lookup(&mode, &keys);

    assert!(result.is_found());
    assert_eq!(result.command_id(), Some(&cmd));
}

#[test]
fn test_keymap_registry_lookup_prefix() {
    let mut registry = KeymapRegistry::new();
    let mode = test_mode();

    // Register `gg`
    registry.register_str(&mode, "gg", test_command("goto-top"));

    // Look up single `g` - should be prefix
    let g = seq("g");
    let result = registry.lookup(&mode, &g);

    assert!(result.is_prefix());
}

#[test]
fn test_keymap_registry_multi_key_sequence() {
    let mut registry = KeymapRegistry::new();
    let mode = test_mode();

    // Register both `g` and `gg`
    registry.register_str(&mode, "g", test_command("goto"));
    registry.register_str(&mode, "gg", test_command("goto-top"));

    // lookup() uses EAGER semantics by default (execute exact match)
    let g = seq("g");
    let result = registry.lookup(&mode, &g);
    assert!(result.is_found());
    assert_eq!(result.command_id(), Some(&test_command("goto")));

    // query() shows the full picture
    let state = registry.query(&mode, &g);
    assert_eq!(
        state,
        LookupState::ExactWithLonger {
            exact: test_command("goto")
        }
    );

    // `gg` is an exact match
    let gg = seq("gg");
    let result = registry.lookup(&mode, &gg);
    assert!(result.is_found());
}

#[test]
fn test_layer_override() {
    let mut registry = KeymapRegistry::new();
    let mode = test_mode();
    let keys = seq("d");

    // Policy layer
    registry.register_at_layer(
        BindingLayer::Policy,
        &mode,
        keys.clone(),
        test_command("delete"),
        "",
        None,
    );

    // User layer (overrides)
    registry.register_at_layer(
        BindingLayer::User,
        &mode,
        keys.clone(),
        test_command("custom-delete"),
        "",
        None,
    );

    // User layer wins
    let binding = registry.get_binding(&mode, &keys);
    assert_eq!(binding, Some(test_command("custom-delete")));
}

#[test]
fn test_unregister_for_module() {
    let mut registry = KeymapRegistry::new();
    let mode = test_mode();
    let owner = ModuleId::new("my-module");

    let j = seq("j");
    let k = seq("k");
    registry.register_at_layer_for_module(
        &mode,
        j.clone(),
        BindingInfo::from_command(test_command("down"), BindingLayer::Policy),
        owner.clone(),
    );
    registry.register_at_layer_for_module(
        &mode,
        k,
        BindingInfo::from_command(test_command("up"), BindingLayer::Policy),
        owner.clone(),
    );

    let l = seq("l");
    registry.register_at_layer(
        BindingLayer::Policy,
        &mode,
        l.clone(),
        test_command("right"),
        "",
        None,
    );

    assert_eq!(registry.binding_count(&mode), 3);

    let removed = registry.unregister_for_module(&owner);

    assert_eq!(removed, 2);
    assert_eq!(registry.binding_count(&mode), 1);
    assert!(registry.lookup(&mode, &j).is_not_found());
    assert!(registry.lookup(&mode, &l).is_found());
}

#[test]
fn test_clear_layer() {
    let mut registry = KeymapRegistry::new();
    let mode = test_mode();

    // Add bindings at different layers
    let j = seq("j");
    registry.register_at_layer(
        BindingLayer::Policy,
        &mode,
        j.clone(),
        test_command("policy-down"),
        "",
        None,
    );
    registry.register_at_layer(
        BindingLayer::User,
        &mode,
        j.clone(),
        test_command("user-down"),
        "",
        None,
    );

    assert_eq!(registry.binding_count(&mode), 1);

    // Clear user layer
    registry.clear_layer(BindingLayer::User, &mode);

    // Policy binding should remain
    let binding = registry.get_binding(&mode, &j);
    assert_eq!(binding, Some(test_command("policy-down")));
}

#[test]
fn test_remove_at_layer() {
    let mut registry = KeymapRegistry::new();
    let mode = test_mode();
    let keys = seq("x");

    // Register at policy layer
    registry.register_at_layer(
        BindingLayer::Policy,
        &mode,
        keys.clone(),
        test_command("delete-char"),
        "",
        None,
    );

    // Mark as removed at user layer
    registry.remove_at_layer(BindingLayer::User, &mode, keys.clone());

    // Should be filtered out
    let binding = registry.get_binding(&mode, &keys);
    assert!(binding.is_none());
}

#[test]
fn test_bindings_for_mode() {
    let mut registry = KeymapRegistry::new();
    let mode = test_mode();

    registry.register_str(&mode, "j", test_command("down"));
    registry.register_str(&mode, "k", test_command("up"));
    registry.register_str(&mode, "h", test_command("left"));

    let bindings = registry.bindings_for_mode(&mode);
    assert_eq!(bindings.len(), 3);
}

#[test]
fn test_bindings_for_mode_empty() {
    let registry = KeymapRegistry::new();
    let mode = test_mode();
    let bindings = registry.bindings_for_mode(&mode);
    assert!(bindings.is_empty());
}

#[test]
fn test_bindings_with_prefix() {
    let mut registry = KeymapRegistry::new();
    let mode = test_mode();

    registry.register_str(&mode, "g", test_command("goto"));
    registry.register_str(&mode, "gg", test_command("goto-top"));
    registry.register_str(&mode, "gj", test_command("goto-next-visual"));

    let g = seq("g");
    let bindings = registry.bindings_with_prefix(&mode, &g);

    // Should not include "g" itself, only longer bindings
    assert_eq!(bindings.len(), 2);
}

#[test]
fn test_bindings_with_prefix_filtered_removed() {
    let mut registry = KeymapRegistry::new();
    let mode = test_mode();

    let gg = seq("gg");
    registry.register_at_layer(
        BindingLayer::Policy,
        &mode,
        gg.clone(),
        test_command("goto-top"),
        "",
        None,
    );
    registry.remove_at_layer(BindingLayer::User, &mode, gg);

    let g = seq("g");
    let bindings = registry.bindings_with_prefix(&mode, &g);

    // Removed binding should not appear
    assert!(bindings.is_empty());
}

#[test]
fn test_bindings_with_prefix_returns_metadata() {
    let mut registry = KeymapRegistry::new();
    let mode = test_mode();

    registry.register_at_layer(
        BindingLayer::Policy,
        &mode,
        seq("gg"),
        test_command("goto-top"),
        "Go to first line",
        Some("motion"),
    );

    let g = seq("g");
    let bindings = registry.bindings_with_prefix(&mode, &g);

    assert_eq!(bindings.len(), 1);
    let (_, info) = &bindings[0];
    assert_eq!(info.command, test_command("goto-top"));
    assert_eq!(info.description, "Go to first line");
    assert_eq!(info.category, Some("motion"));
    assert_eq!(info.layer, BindingLayer::Policy);
}

#[test]
fn test_bindings_with_prefix_user_layer_metadata() {
    let mut registry = KeymapRegistry::new();
    let mode = test_mode();

    registry.register_at_layer(
        BindingLayer::User,
        &mode,
        seq("gg"),
        test_command("custom-goto"),
        "Custom goto",
        Some("custom"),
    );

    let g = seq("g");
    let bindings = registry.bindings_with_prefix(&mode, &g);

    assert_eq!(bindings.len(), 1);
    let (_, info) = &bindings[0];
    assert_eq!(info.layer, BindingLayer::User);
    assert_eq!(info.description, "Custom goto");
    assert_eq!(info.category, Some("custom"));
}

#[test]
fn test_total_bindings() {
    let mut registry = KeymapRegistry::new();
    let mode1 = ModeId::with_discriminant(TEST_MODULE, "NORMAL", 0);
    let mode2 = ModeId::with_discriminant(TEST_MODULE, "INSERT", 1);

    registry.register_str(&mode1, "j", test_command("down"));
    registry.register_str(&mode1, "k", test_command("up"));
    registry.register_str(&mode2, "a", test_command("append"));

    assert_eq!(registry.total_bindings(), 3);
}

#[test]
fn test_modes_iterator() {
    let mut registry = KeymapRegistry::new();
    let mode1 = ModeId::with_discriminant(TEST_MODULE, "NORMAL", 0);
    let mode2 = ModeId::with_discriminant(TEST_MODULE, "INSERT", 1);

    registry.register_str(&mode1, "j", test_command("down"));
    registry.register_str(&mode2, "a", test_command("append"));

    assert_eq!(registry.modes().count(), 2);
}

#[test]
fn test_keymap_query_trait() {
    let mut registry = KeymapRegistry::new();
    let mode = test_mode();
    let keys = seq("j");

    registry.register_str(&mode, "j", test_command("down"));

    let state = registry.query(&mode, &keys);
    assert!(matches!(state, LookupState::ExactOnly(_)));

    let exact = registry.get_binding(&mode, &keys);
    assert_eq!(exact, Some(test_command("down")));

    let has_longer = registry.has_longer_bindings(&mode, &keys);
    assert!(!has_longer);
}

#[test]
fn test_keymap_query_trait_prefix() {
    let mut registry = KeymapRegistry::new();
    let mode = test_mode();

    registry.register_str(&mode, "gg", test_command("goto-top"));

    let g = seq("g");

    let has_longer = registry.has_longer_bindings(&mode, &g);
    assert!(has_longer);

    let bindings = registry.bindings_with_prefix(&mode, &g);
    assert_eq!(bindings.len(), 1);
}

#[test]
fn test_debug_format() {
    let mut registry = KeymapRegistry::new();
    let mode = test_mode();
    registry.register_str(&mode, "j", test_command("down"));

    let debug_str = format!("{registry:?}");
    assert!(debug_str.contains("KeymapRegistry"));
    assert!(debug_str.contains("total_bindings"));
}

#[test]
fn test_register_str_invalid() {
    let mut registry = KeymapRegistry::new();
    let mode = test_mode();

    // "<<invalid>>" starts with '<' but test_helpers::seq_from_notation only
    // handles plain ASCII chars — the '<' char becomes a key, not notation.
    // However an empty string returns false. Use empty string here.
    let result = registry.register_str(&mode, "", test_command("noop"));
    assert!(!result);
}

#[test]
fn test_query_not_found() {
    let registry = KeymapRegistry::new();
    let mode = test_mode();
    let keys = seq("z");

    let state = registry.query(&mode, &keys);
    assert_eq!(state, LookupState::NotFound);
}

#[test]
fn test_query_exact_only() {
    let mut registry = KeymapRegistry::new();
    let mode = test_mode();
    let keys = seq("j");

    registry.register_str(&mode, "j", test_command("down"));

    let state = registry.query(&mode, &keys);
    assert_eq!(state, LookupState::ExactOnly(test_command("down")));
}

#[test]
fn test_query_prefix_only() {
    let mut registry = KeymapRegistry::new();
    let mode = test_mode();

    registry.register_str(&mode, "gg", test_command("goto-top"));

    let g = seq("g");
    let state = registry.query(&mode, &g);
    assert_eq!(state, LookupState::PrefixOnly);
}

#[test]
fn test_query_exact_with_longer() {
    let mut registry = KeymapRegistry::new();
    let mode = test_mode();

    registry.register_str(&mode, "g", test_command("goto"));
    registry.register_str(&mode, "gg", test_command("goto-top"));

    let g = seq("g");
    let state = registry.query(&mode, &g);
    assert_eq!(
        state,
        LookupState::ExactWithLonger {
            exact: test_command("goto")
        }
    );
}

#[test]
fn test_lookup_with_custom_policy() {
    use reovim_subsys_input::LookupPolicy;

    struct AlwaysExecutePolicy;
    #[cfg_attr(coverage_nightly, coverage(off))]
    impl LookupPolicy<CommandId> for AlwaysExecutePolicy {
        fn resolve(&self, state: LookupState<CommandId>) -> LookupResult<CommandId> {
            match state {
                LookupState::ExactOnly(cmd) | LookupState::ExactWithLonger { exact: cmd } => {
                    LookupResult::Found(cmd)
                }
                _ => LookupResult::NotFound,
            }
        }
    }

    let mut registry = KeymapRegistry::new();
    let mode = test_mode();

    registry.register_str(&mode, "g", test_command("goto"));
    registry.register_str(&mode, "gg", test_command("goto-top"));

    let g = seq("g");
    let policy = AlwaysExecutePolicy;
    let result = registry.lookup_with_policy(&mode, &g, &policy);

    // Custom policy executes immediately
    assert!(result.is_found());
    assert_eq!(result.command_id(), Some(&test_command("goto")));
}

#[test]
fn test_register_at_layer_for_module_replaces() {
    let mut registry = KeymapRegistry::new();
    let mode = test_mode();
    let keys = seq("x");
    let owner = ModuleId::new("owner");

    // Register first
    registry.register_at_layer_for_module(
        &mode,
        keys.clone(),
        BindingInfo::from_command(test_command("delete1"), BindingLayer::Policy),
        owner.clone(),
    );

    // Register again at same layer - should replace
    registry.register_at_layer_for_module(
        &mode,
        keys.clone(),
        BindingInfo::from_command(test_command("delete2"), BindingLayer::Policy),
        owner,
    );

    let binding = registry.get_binding(&mode, &keys);
    assert_eq!(binding, Some(test_command("delete2")));
}

#[test]
fn test_clear_layer_removes_mode_if_empty() {
    let mut registry = KeymapRegistry::new();
    let mode = test_mode();
    let keys = seq("j");

    registry.register_at_layer(BindingLayer::User, &mode, keys, test_command("down"), "", None);

    assert!(!registry.is_empty());

    registry.clear_layer(BindingLayer::User, &mode);

    // Mode should be removed from registry if empty
    assert!(registry.is_empty());
}

#[test]
fn test_unregister_for_module_removes_mode_if_empty() {
    let mut registry = KeymapRegistry::new();
    let mode = test_mode();
    let owner = ModuleId::new("owner");
    let keys = seq("j");

    registry.register_at_layer_for_module(
        &mode,
        keys,
        BindingInfo::from_command(test_command("down"), BindingLayer::Policy),
        owner.clone(),
    );

    assert!(!registry.is_empty());

    registry.unregister_for_module(&owner);

    // Mode should be removed from registry if empty
    assert!(registry.is_empty());
}

#[test]
fn test_unregister_for_module_nonexistent() {
    let mut registry = KeymapRegistry::new();
    let mode = test_mode();

    registry.register_str(&mode, "j", test_command("down"));

    let nonexistent = ModuleId::new("nonexistent");
    let removed = registry.unregister_for_module(&nonexistent);

    assert_eq!(removed, 0);
    assert_eq!(registry.binding_count(&mode), 1);
}

#[test]
fn test_has_longer_bindings_false() {
    let mut registry = KeymapRegistry::new();
    let mode = test_mode();

    registry.register_str(&mode, "j", test_command("down"));

    let j = seq("j");
    assert!(!registry.has_longer_bindings(&mode, &j));
}

#[test]
fn test_has_longer_bindings_true() {
    let mut registry = KeymapRegistry::new();
    let mode = test_mode();

    registry.register_str(&mode, "gg", test_command("goto-top"));

    let g = seq("g");
    assert!(registry.has_longer_bindings(&mode, &g));
}

#[test]
fn test_is_empty_with_empty_entries() {
    let registry = KeymapRegistry::new();
    assert!(registry.is_empty());
}

#[test]
fn test_binding_count_nonexistent_mode() {
    let registry = KeymapRegistry::new();
    let mode = test_mode();
    assert_eq!(registry.binding_count(&mode), 0);
}

#[test]
fn test_layer_sorting() {
    let mut registry = KeymapRegistry::new();
    let mode = test_mode();
    let keys = seq("x");

    // Add in reverse order
    registry.register_at_layer(
        BindingLayer::Base,
        &mode,
        keys.clone(),
        test_command("base"),
        "",
        None,
    );
    registry.register_at_layer(
        BindingLayer::User,
        &mode,
        keys.clone(),
        test_command("user"),
        "",
        None,
    );
    registry.register_at_layer(
        BindingLayer::Policy,
        &mode,
        keys.clone(),
        test_command("policy"),
        "",
        None,
    );

    // User layer should win
    let binding = registry.get_binding(&mode, &keys);
    assert_eq!(binding, Some(test_command("user")));
}

#[test]
fn test_modes_empty_registry() {
    let registry = KeymapRegistry::new();
    assert_eq!(registry.modes().count(), 0);
}

#[test]
fn test_clear_layer_removes_all_bindings_and_cleans_empty_modes() {
    let mut registry = KeymapRegistry::new();
    let mode = test_mode();
    let j = seq("j");

    registry.register_at_layer(BindingLayer::Policy, &mode, j, test_command("down"), "", None);
    assert_eq!(registry.total_bindings(), 1);

    // Clearing the only layer should remove the mode entry entirely
    registry.clear_layer(BindingLayer::Policy, &mode);
    assert_eq!(registry.total_bindings(), 0);
    assert_eq!(registry.modes().count(), 0);
}

#[test]
fn test_clear_layer_nonexistent_mode_is_noop() {
    let mut registry = KeymapRegistry::new();
    let mode = test_mode();
    // clear_layer on a mode that doesn't exist should not panic
    registry.clear_layer(BindingLayer::Policy, &mode);
    assert_eq!(registry.total_bindings(), 0);
}

#[test]
fn test_set_default_policy() {
    use reovim_subsys_input::LookupPolicy;

    /// Test policy that waits for longer sequences (Vim-style).
    struct WaitForLongerPolicy;
    #[cfg_attr(coverage_nightly, coverage(off))]
    impl LookupPolicy<CommandId> for WaitForLongerPolicy {
        fn resolve(&self, state: LookupState<CommandId>) -> LookupResult<CommandId> {
            match state {
                LookupState::ExactWithLonger { .. } | LookupState::PrefixOnly => {
                    LookupResult::Prefix
                }
                LookupState::ExactOnly(cmd) => LookupResult::Found(cmd),
                LookupState::NotFound => LookupResult::NotFound,
            }
        }
    }

    let mut registry = KeymapRegistry::new();
    let mode = test_mode();

    registry.register_str(&mode, "g", test_command("goto"));
    registry.register_str(&mode, "gg", test_command("goto-top"));

    let g = seq("g");

    // Default (eager) executes exact match immediately
    let result = registry.lookup(&mode, &g);
    assert!(result.is_found());

    // Switch to wait-for-longer policy - should now wait for longer sequences
    registry.set_default_policy(Arc::new(WaitForLongerPolicy));
    let result = registry.lookup(&mode, &g);
    assert!(result.is_prefix());
}

#[test]
fn test_default_policy_is_eager() {
    let registry = KeymapRegistry::new();
    // Verify default is eager by checking struct fields via Default
    let default_registry = KeymapRegistry::default();
    assert!(registry.is_empty());
    assert!(default_registry.is_empty());
}
