use reovim_kernel::api::v1::{CommandId, ModeId, ModuleId};

use crate::{
    BindingLayer, EagerLookupPolicy, KeyLookupPolicy, KeyLookupResult, KeyLookupState, KeySequence,
    KeymapQuery,
};

fn test_module() -> ModuleId {
    ModuleId::new("test")
}

fn test_command(name: &'static str) -> CommandId {
    CommandId::new(test_module(), name)
}

fn test_mode(name: &'static str) -> ModeId {
    ModeId::new(test_module(), name)
}

// ========================================================================
// BindingLayer tests
// ========================================================================

#[test]
fn test_binding_layer_order() {
    // User > Policy > Base
    assert!(BindingLayer::User > BindingLayer::Policy);
    assert!(BindingLayer::Policy > BindingLayer::Base);
}

#[test]
fn test_binding_layer_all() {
    let layers = BindingLayer::all();
    assert_eq!(layers.len(), 3);
    assert_eq!(layers[0], BindingLayer::Base);
    assert_eq!(layers[1], BindingLayer::Policy);
    assert_eq!(layers[2], BindingLayer::User);
}

#[test]
fn test_binding_layer_names() {
    assert_eq!(BindingLayer::Base.name(), "base");
    assert_eq!(BindingLayer::Policy.name(), "policy");
    assert_eq!(BindingLayer::User.name(), "user");
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_binding_layer_debug() {
    let debug = format!("{:?}", BindingLayer::Base);
    assert_eq!(debug, "Base");
    let debug = format!("{:?}", BindingLayer::Policy);
    assert_eq!(debug, "Policy");
    let debug = format!("{:?}", BindingLayer::User);
    assert_eq!(debug, "User");
}

#[test]
fn test_binding_layer_copy_clone() {
    let layer = BindingLayer::Policy;
    let cloned = layer;
    assert_eq!(layer, cloned);
}

#[test]
fn test_binding_layer_hash() {
    use std::collections::HashSet;
    let mut set = HashSet::new();
    set.insert(BindingLayer::Base);
    set.insert(BindingLayer::Policy);
    set.insert(BindingLayer::User);
    assert_eq!(set.len(), 3);
    // Duplicate insert
    set.insert(BindingLayer::Base);
    assert_eq!(set.len(), 3);
}

// ========================================================================
// KeyLookupState tests
// ========================================================================

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_key_lookup_state_debug() {
    let state = KeyLookupState::NotFound;
    let _ = format!("{state:?}");
}

#[test]
fn test_key_lookup_state_exact_only() {
    let cmd = test_command("delete");
    let state = KeyLookupState::ExactOnly(cmd.clone());
    assert_eq!(state, KeyLookupState::ExactOnly(cmd));
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_key_lookup_state_exact_with_longer() {
    let cmd = test_command("delete");
    let state = KeyLookupState::ExactWithLonger { exact: cmd.clone() };
    if let KeyLookupState::ExactWithLonger { exact } = &state {
        assert_eq!(exact, &cmd);
    } else {
        panic!("expected ExactWithLonger");
    }
}

#[test]
fn test_key_lookup_state_prefix_only() {
    let state = KeyLookupState::PrefixOnly;
    assert_eq!(state, KeyLookupState::PrefixOnly);
}

#[test]
fn test_key_lookup_state_equality() {
    let cmd1 = test_command("cmd1");
    let cmd2 = test_command("cmd2");

    assert_eq!(KeyLookupState::ExactOnly(cmd1.clone()), KeyLookupState::ExactOnly(cmd1));
    assert_ne!(KeyLookupState::ExactOnly(cmd2.clone()), KeyLookupState::NotFound);
    assert_ne!(KeyLookupState::PrefixOnly, KeyLookupState::ExactWithLonger { exact: cmd2 });
}

// ========================================================================
// KeyLookupResult tests
// ========================================================================

#[test]
fn test_key_lookup_result_found() {
    let cmd = test_command("delete");
    let result = KeyLookupResult::Found(cmd.clone());
    assert!(result.is_found());
    assert!(!result.is_prefix());
    assert!(!result.is_not_found());
    assert_eq!(result.command_id(), Some(&cmd));
}

#[test]
fn test_key_lookup_result_prefix() {
    let result = KeyLookupResult::Prefix;
    assert!(!result.is_found());
    assert!(result.is_prefix());
    assert!(!result.is_not_found());
    assert_eq!(result.command_id(), None);
}

#[test]
fn test_key_lookup_result_not_found() {
    let result = KeyLookupResult::NotFound;
    assert!(!result.is_found());
    assert!(!result.is_prefix());
    assert!(result.is_not_found());
    assert_eq!(result.command_id(), None);
}

#[test]
fn test_key_lookup_result_equality() {
    let cmd = test_command("cmd");
    assert_eq!(KeyLookupResult::Found(cmd.clone()), KeyLookupResult::Found(cmd));
    assert_eq!(KeyLookupResult::Prefix, KeyLookupResult::Prefix);
    assert_eq!(KeyLookupResult::NotFound, KeyLookupResult::NotFound);
    assert_ne!(KeyLookupResult::Prefix, KeyLookupResult::NotFound);
}

// ========================================================================
// EagerLookupPolicy tests
// ========================================================================

#[test]
fn test_eager_policy_exact_only_returns_found() {
    let policy = EagerLookupPolicy;
    let cmd = test_command("delete_char");
    let result = policy.resolve(KeyLookupState::ExactOnly(cmd.clone()));
    assert_eq!(result, KeyLookupResult::Found(cmd));
}

#[test]
fn test_eager_policy_exact_with_longer_returns_found() {
    // Key difference from Vim: eagerly execute even when longer bindings exist
    let policy = EagerLookupPolicy;
    let cmd = test_command("delete_op");
    let result = policy.resolve(KeyLookupState::ExactWithLonger { exact: cmd.clone() });
    assert_eq!(result, KeyLookupResult::Found(cmd));
}

#[test]
fn test_eager_policy_prefix_only_returns_prefix() {
    let policy = EagerLookupPolicy;
    let result = policy.resolve(KeyLookupState::PrefixOnly);
    assert_eq!(result, KeyLookupResult::Prefix);
}

#[test]
fn test_eager_policy_not_found_returns_not_found() {
    let policy = EagerLookupPolicy;
    let result = policy.resolve(KeyLookupState::NotFound);
    assert_eq!(result, KeyLookupResult::NotFound);
}

// ========================================================================
// KeymapQuery trait default methods tests
// ========================================================================

/// Mock keymap for testing default trait methods.
struct MockKeymap {
    state: KeyLookupState,
}

impl MockKeymap {
    fn with_state(state: KeyLookupState) -> Self {
        Self { state }
    }
}

impl KeymapQuery for MockKeymap {
    fn query(&self, _mode: &ModeId, _keys: &KeySequence) -> KeyLookupState {
        self.state.clone()
    }
}

#[test]
fn test_keymap_query_has_longer_bindings_exact_with_longer() {
    let cmd = test_command("cmd");
    let keymap = MockKeymap::with_state(KeyLookupState::ExactWithLonger { exact: cmd });
    let mode = test_mode("normal");
    let keys = KeySequence::new();
    assert!(keymap.has_longer_bindings(&mode, &keys));
}

#[test]
fn test_keymap_query_has_longer_bindings_prefix_only() {
    let keymap = MockKeymap::with_state(KeyLookupState::PrefixOnly);
    let mode = test_mode("normal");
    let keys = KeySequence::new();
    assert!(keymap.has_longer_bindings(&mode, &keys));
}

#[test]
fn test_keymap_query_has_longer_bindings_exact_only_returns_false() {
    let cmd = test_command("cmd");
    let keymap = MockKeymap::with_state(KeyLookupState::ExactOnly(cmd));
    let mode = test_mode("normal");
    let keys = KeySequence::new();
    assert!(!keymap.has_longer_bindings(&mode, &keys));
}

#[test]
fn test_keymap_query_has_longer_bindings_not_found_returns_false() {
    let keymap = MockKeymap::with_state(KeyLookupState::NotFound);
    let mode = test_mode("normal");
    let keys = KeySequence::new();
    assert!(!keymap.has_longer_bindings(&mode, &keys));
}

#[test]
fn test_keymap_query_get_exact_exact_only() {
    let cmd = test_command("delete");
    let keymap = MockKeymap::with_state(KeyLookupState::ExactOnly(cmd.clone()));
    let mode = test_mode("normal");
    let keys = KeySequence::new();
    assert_eq!(keymap.get_exact(&mode, &keys), Some(cmd));
}

#[test]
fn test_keymap_query_get_exact_exact_with_longer() {
    let cmd = test_command("delete");
    let keymap = MockKeymap::with_state(KeyLookupState::ExactWithLonger { exact: cmd.clone() });
    let mode = test_mode("normal");
    let keys = KeySequence::new();
    assert_eq!(keymap.get_exact(&mode, &keys), Some(cmd));
}

#[test]
fn test_keymap_query_get_exact_prefix_only_returns_none() {
    let keymap = MockKeymap::with_state(KeyLookupState::PrefixOnly);
    let mode = test_mode("normal");
    let keys = KeySequence::new();
    assert_eq!(keymap.get_exact(&mode, &keys), None);
}

#[test]
fn test_keymap_query_get_exact_not_found_returns_none() {
    let keymap = MockKeymap::with_state(KeyLookupState::NotFound);
    let mode = test_mode("normal");
    let keys = KeySequence::new();
    assert_eq!(keymap.get_exact(&mode, &keys), None);
}

#[test]
fn test_keymap_query_bindings_with_prefix_default_returns_empty() {
    let keymap = MockKeymap::with_state(KeyLookupState::NotFound);
    let mode = test_mode("normal");
    let keys = KeySequence::new();
    assert!(keymap.bindings_with_prefix(&mode, &keys).is_empty());
}

// ========================================================================
// KeyLookupPolicy trait object safety
// ========================================================================

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_key_lookup_policy_is_object_safe() {
    fn _accepts_ref(_: &dyn KeyLookupPolicy) {}
    fn _accepts_box(_: Box<dyn KeyLookupPolicy>) {}
}

// ========================================================================
// KeymapQuery trait object safety
// ========================================================================

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_keymap_query_is_object_safe() {
    fn _accepts_ref(_: &dyn KeymapQuery) {}
    fn _accepts_box(_: Box<dyn KeymapQuery>) {}
}
