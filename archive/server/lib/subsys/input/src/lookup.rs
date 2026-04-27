//! Generic keybinding lookup primitives.
//!
//! These types are the domain-neutral mechanism for keybinding lookup.
//! They are generic over the command type `C` so the same machinery works
//! with `CommandId`, string labels, or any future command representation.
//!
//! `KeymapQuery<C>` uses `InputSequence` (the new opaque byte-sequence key)
//! instead of the old `KeySequence` string-based key.  This aligns with
//! the Plan-14 invariant that the server registry stores opaque payloads.

use reovim_input_codec::InputSequence;

/// Pure mechanism facts about what bindings exist for an input sequence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LookupState<C> {
    /// Exact match exists, with no longer bindings.
    ExactOnly(C),
    /// Exact match exists and longer bindings also exist.
    ExactWithLonger {
        /// The command for the exact match.
        exact: C,
    },
    /// No exact match, but longer bindings exist.
    PrefixOnly,
    /// No binding facts matched.
    NotFound,
}

/// Final decision after a policy interprets lookup facts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LookupResult<C> {
    /// Execute this command.
    Found(C),
    /// Wait for more input.
    Prefix,
    /// Nothing matched.
    NotFound,
}

impl<C> LookupResult<C> {
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

    /// Returns the command if this is a `Found` result.
    #[must_use]
    pub const fn as_found(&self) -> Option<&C> {
        match self {
            Self::Found(cmd) => Some(cmd),
            Self::Prefix | Self::NotFound => None,
        }
    }

    /// Returns the command if this is a `Found` result.
    ///
    /// Alias for [`as_found`](Self::as_found).
    #[must_use]
    pub const fn command_id(&self) -> Option<&C> {
        self.as_found()
    }
}

/// Policy hook for interpreting key lookup facts.
pub trait LookupPolicy<C>: Send + Sync {
    /// Interpret lookup facts and produce a final decision.
    fn resolve(&self, state: LookupState<C>) -> LookupResult<C>;
}

/// Eager policy: execute exact matches immediately without waiting for
/// potentially longer bindings.
#[derive(Debug, Clone, Copy, Default)]
pub struct EagerLookupPolicy;

impl<C> LookupPolicy<C> for EagerLookupPolicy {
    fn resolve(&self, state: LookupState<C>) -> LookupResult<C> {
        match state {
            LookupState::ExactOnly(cmd) | LookupState::ExactWithLonger { exact: cmd } => {
                LookupResult::Found(cmd)
            }
            LookupState::PrefixOnly => LookupResult::Prefix,
            LookupState::NotFound => LookupResult::NotFound,
        }
    }
}

/// Trait for querying keybindings by `InputSequence`.
///
/// Implementations are typically the server's keymap registry.
/// The generic parameter `C` is the command type returned on a match
/// (e.g., `CommandId`).
pub trait KeymapQuery<C>: Send + Sync {
    /// Query keybinding facts for a mode ID and an opaque input sequence.
    ///
    /// The `mode` parameter is an opaque string identifier (module-scoped
    /// mode name) rather than a kernel `ModeId` so that this contract
    /// stays free of kernel policy types.
    fn query(&self, mode: &str, keys: &InputSequence) -> LookupState<C>;

    /// Check if longer bindings exist for an input sequence.
    fn has_longer_bindings(&self, mode: &str, keys: &InputSequence) -> bool {
        matches!(
            self.query(mode, keys),
            LookupState::ExactWithLonger { .. } | LookupState::PrefixOnly
        )
    }

    /// Get the exact binding for an input sequence.
    fn get_exact(&self, mode: &str, keys: &InputSequence) -> Option<C> {
        match self.query(mode, keys) {
            LookupState::ExactOnly(cmd) | LookupState::ExactWithLonger { exact: cmd } => Some(cmd),
            LookupState::PrefixOnly | LookupState::NotFound => None,
        }
    }
}
