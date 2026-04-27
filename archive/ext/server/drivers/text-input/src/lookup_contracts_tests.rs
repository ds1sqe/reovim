//! Tests for `BindingLayer`, `KeyLookupState`, `KeyLookupResult`, `EagerLookupPolicy`,
//! and `KeymapQuery`.

use {super::*, reovim_kernel::api::v1::ModuleId};

fn test_module() -> ModuleId {
    ModuleId::new("test")
}

fn test_command(name: &'static str) -> reovim_kernel::api::v1::CommandId {
    reovim_kernel::api::v1::CommandId::new(test_module(), name)
}

fn test_mode(name: &'static str) -> reovim_kernel::api::v1::ModeId {
    reovim_kernel::api::v1::ModeId::new(test_module(), name)
}

struct MockKeymap {
    state: KeyLookupState,
}

impl KeymapQuery for MockKeymap {
    fn query(
        &self,
        _mode: &reovim_kernel::api::v1::ModeId,
        _keys: &crate::KeySequence,
    ) -> KeyLookupState {
        self.state.clone()
    }
}

#[test]
fn binding_layer_helpers_report_expected_priority_and_names() {
    assert!(BindingLayer::User > BindingLayer::Policy);
    assert!(BindingLayer::Policy > BindingLayer::Base);
    assert_eq!(
        BindingLayer::all(),
        [BindingLayer::Base, BindingLayer::Policy, BindingLayer::User]
    );
    assert_eq!(BindingLayer::Base.name(), "base");
    assert_eq!(BindingLayer::Policy.name(), "policy");
    assert_eq!(BindingLayer::User.name(), "user");
}

#[test]
fn key_lookup_result_helper_methods_cover_all_variants() {
    let found = KeyLookupResult::Found(test_command("delete"));
    assert!(found.is_found());
    assert_eq!(found.command_id().unwrap().name(), "delete");

    let prefix = KeyLookupResult::Prefix;
    assert!(prefix.is_prefix());
    assert!(prefix.command_id().is_none());

    let missing = KeyLookupResult::NotFound;
    assert!(missing.is_not_found());
    assert!(missing.command_id().is_none());
}

#[test]
fn eager_policy_executes_exact_matches_and_waits_for_prefixes() {
    let policy = EagerLookupPolicy;
    let cmd = test_command("delete");
    assert_eq!(
        policy.resolve(KeyLookupState::ExactOnly(cmd.clone())),
        KeyLookupResult::Found(cmd.clone())
    );
    assert_eq!(
        policy.resolve(KeyLookupState::ExactWithLonger { exact: cmd.clone() }),
        KeyLookupResult::Found(cmd)
    );
    assert_eq!(policy.resolve(KeyLookupState::PrefixOnly), KeyLookupResult::Prefix);
    assert_eq!(policy.resolve(KeyLookupState::NotFound), KeyLookupResult::NotFound);
}

#[test]
fn keymap_query_default_helpers_delegate_to_query_facts() {
    use crate::KeySequence;

    let mode = test_mode("normal");
    let keys = KeySequence::parse("gg").unwrap();

    let exact = MockKeymap {
        state: KeyLookupState::ExactOnly(test_command("goto-top")),
    };
    assert!(!exact.has_longer_bindings(&mode, &keys));
    assert_eq!(exact.get_exact(&mode, &keys).unwrap().name(), "goto-top");
    assert!(exact.bindings_with_prefix(&mode, &keys).is_empty());

    let longer = MockKeymap {
        state: KeyLookupState::ExactWithLonger {
            exact: test_command("delete-op"),
        },
    };
    assert!(longer.has_longer_bindings(&mode, &keys));

    let prefix = MockKeymap {
        state: KeyLookupState::PrefixOnly,
    };
    assert!(prefix.has_longer_bindings(&mode, &keys));
    assert!(prefix.get_exact(&mode, &keys).is_none());
}
