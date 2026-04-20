use {
    reovim_driver_text_input::{KeyLookupPolicy, KeyLookupResult, KeyLookupState},
    reovim_kernel::api::v1::{CommandId, ModuleId},
};

use crate::vim_lookup_policy::*;

fn test_command(name: &'static str) -> CommandId {
    CommandId::new(ModuleId::new("test"), name)
}

#[test]
fn test_vim_policy_exact_only_returns_found() {
    let policy = VimLookupPolicy;
    let cmd = test_command("delete_char");
    let result = policy.resolve(KeyLookupState::ExactOnly(cmd.clone()));
    assert_eq!(result, KeyLookupResult::Found(cmd));
}

#[test]
fn test_vim_policy_exact_with_longer_returns_prefix() {
    let policy = VimLookupPolicy;
    let cmd = test_command("delete_op");
    let result = policy.resolve(KeyLookupState::ExactWithLonger { exact: cmd });
    assert_eq!(result, KeyLookupResult::Prefix);
}

#[test]
fn test_vim_policy_prefix_only_returns_prefix() {
    let policy = VimLookupPolicy;
    let result = policy.resolve(KeyLookupState::PrefixOnly);
    assert_eq!(result, KeyLookupResult::Prefix);
}

#[test]
fn test_vim_policy_not_found_returns_not_found() {
    let policy = VimLookupPolicy;
    let result = policy.resolve(KeyLookupState::NotFound);
    assert_eq!(result, KeyLookupResult::NotFound);
}

#[test]
fn test_vim_policy_debug() {
    let policy = VimLookupPolicy;
    let debug = format!("{policy:?}");
    assert!(debug.contains("VimLookupPolicy"));
}

#[test]
fn test_vim_policy_clone() {
    let policy = VimLookupPolicy;
    let cloned = policy;
    // Both should produce same results
    let cmd = test_command("test");
    assert_eq!(
        policy.resolve(KeyLookupState::ExactOnly(cmd.clone())),
        cloned.resolve(KeyLookupState::ExactOnly(cmd))
    );
}

#[test]
fn test_vim_policy_implements_default() {
    // VimLookupPolicy derives Default -- verify it works
    fn assert_default<T: Default>() {}
    assert_default::<VimLookupPolicy>();
}
