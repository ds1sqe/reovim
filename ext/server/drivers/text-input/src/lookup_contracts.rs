//! Non-generic key lookup contracts for the text-input driver.
//!
//! These are the string-token-based lookup types used with `KeySequence`.
//! Moved from `reovim-subsys-input-contracts` as part of Plan-14 I.6.
//!
//! The domain-neutral GENERIC lookup primitives (`LookupState<C>`,
//! `LookupResult<C>`, `KeymapQuery<C>`) live in `reovim-subsys-input`.
//! This module contains the TEXT-INPUT-SPECIFIC forms that bind to
//! `KeySequence` (notation-token-based).

use reovim_kernel::api::v1::{CommandId, ModeId};

use crate::{BindingInfo, KeySequence};

// Re-export BindingLayer from the mechanism layer so existing code that
// imports it from `crate::BindingLayer` continues to work.
pub use reovim_subsys_input::BindingLayer;

/// Pure mechanism facts about what bindings exist for a key sequence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeyLookupState {
    /// Exact match exists, with no longer bindings.
    ExactOnly(CommandId),
    /// Exact match exists and longer bindings also exist.
    ExactWithLonger {
        /// The command for the exact match.
        exact: CommandId,
    },
    /// No exact match, but longer bindings exist.
    PrefixOnly,
    /// No binding facts matched.
    NotFound,
}

/// Policy hook for interpreting key lookup facts.
pub trait KeyLookupPolicy: Send + Sync {
    /// Interpret key lookup facts.
    fn resolve(&self, state: KeyLookupState) -> KeyLookupResult;
}

/// Final decision after a policy interprets lookup facts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeyLookupResult {
    /// Execute this command.
    Found(CommandId),
    /// Wait for more keys.
    Prefix,
    /// Nothing matched.
    NotFound,
}

impl KeyLookupResult {
    /// Returns `true` if this is a `Found` result.
    #[must_use]
    pub const fn is_found(&self) -> bool {
        matches!(self, Self::Found(_))
    }

    /// Returns `true` if this is a `Prefix` result.
    #[must_use]
    pub const fn is_prefix(&self) -> bool {
        matches!(self, Self::Prefix)
    }

    /// Returns `true` if this is a `NotFound` result.
    #[must_use]
    pub const fn is_not_found(&self) -> bool {
        matches!(self, Self::NotFound)
    }

    /// Returns the command ID if this is a `Found` result.
    #[must_use]
    pub const fn command_id(&self) -> Option<&CommandId> {
        match self {
            Self::Found(cmd) => Some(cmd),
            Self::Prefix | Self::NotFound => None,
        }
    }
}

/// Eager policy: execute exact matches immediately.
#[derive(Debug, Clone, Copy, Default)]
pub struct EagerLookupPolicy;

impl KeyLookupPolicy for EagerLookupPolicy {
    fn resolve(&self, state: KeyLookupState) -> KeyLookupResult {
        match state {
            KeyLookupState::ExactOnly(cmd) | KeyLookupState::ExactWithLonger { exact: cmd } => {
                KeyLookupResult::Found(cmd)
            }
            KeyLookupState::PrefixOnly => KeyLookupResult::Prefix,
            KeyLookupState::NotFound => KeyLookupResult::NotFound,
        }
    }
}

/// Trait for querying keybindings (string-token variant).
pub trait KeymapQuery: Send + Sync {
    /// Query keybinding facts for a mode and key sequence.
    fn query(&self, mode: &ModeId, keys: &KeySequence) -> KeyLookupState;

    /// Check if longer bindings exist for a sequence.
    fn has_longer_bindings(&self, mode: &ModeId, keys: &KeySequence) -> bool {
        matches!(
            self.query(mode, keys),
            KeyLookupState::ExactWithLonger { .. } | KeyLookupState::PrefixOnly
        )
    }

    /// Get the exact binding for a key sequence.
    fn get_exact(&self, mode: &ModeId, keys: &KeySequence) -> Option<CommandId> {
        match self.query(mode, keys) {
            KeyLookupState::ExactOnly(cmd) | KeyLookupState::ExactWithLonger { exact: cmd } => {
                Some(cmd)
            }
            KeyLookupState::PrefixOnly | KeyLookupState::NotFound => None,
        }
    }

    /// Get all bindings extending a prefix.
    fn bindings_with_prefix(
        &self,
        _mode: &ModeId,
        _prefix: &KeySequence,
    ) -> Vec<(KeySequence, BindingInfo)> {
        Vec::new()
    }
}

