//! Vim-style lookup policy.
//!
//! This module defines `VimLookupPolicy`, the Vim-specific policy for
//! interpreting key lookup results. It prefers waiting for longer sequences
//! (e.g., waits for `dd` after `d` is pressed).
//!
//! # Architecture
//!
//! This is POLICY code - it lives in the vim module because it defines
//! Vim-specific behavior. The mechanism types (`KeyLookupPolicy`,
//! `KeyLookupState`, `KeyLookupResult`) live in `driver-input`.

use reovim_driver_input::{KeyLookupPolicy, KeyLookupResult, KeyLookupState};

/// Vim-style lookup policy: prefer longer sequences.
///
/// When an exact match exists AND longer bindings exist (e.g., `d` when `dd`
/// also exists), this policy returns `Prefix` to wait for more keys.
///
/// This is the standard Vim behavior where typing `d` doesn't immediately
/// execute anything - it waits to see if `dd`, `dw`, `d$`, etc. follow.
#[derive(Debug, Clone, Copy, Default)]
pub struct VimLookupPolicy;

impl KeyLookupPolicy for VimLookupPolicy {
    fn resolve(&self, state: KeyLookupState) -> KeyLookupResult {
        match state {
            // Vim: if longer bindings exist, wait for more keys
            KeyLookupState::ExactWithLonger { .. } | KeyLookupState::PrefixOnly => {
                KeyLookupResult::Prefix
            }
            // Only execute if no longer bindings exist
            KeyLookupState::ExactOnly(cmd) => KeyLookupResult::Found(cmd),
            KeyLookupState::NotFound => KeyLookupResult::NotFound,
        }
    }
}

#[cfg(test)]
mod tests {
    use reovim_kernel::api::v1::{CommandId, ModuleId};

    use super::*;

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
        // VimLookupPolicy derives Default — verify it works
        fn assert_default<T: Default>() {}
        assert_default::<VimLookupPolicy>();
    }
}
