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
