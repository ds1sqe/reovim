//! Key lookup types for mechanism/policy separation.
//!
//! This module provides pure mechanism types for key binding lookup:
//!
//! - [`KeyLookupState`]: Reports FACTS about what bindings exist (no policy decisions)
//! - [`BindingLayer`]: Priority levels for layered binding composition
//! - [`KeymapQuery`]: Trait for querying keybindings (implemented by runner)
//!
//! # Architecture
//!
//! The key insight is that bindings and behavior are separate concerns:
//!
//! | Concern | How it works | Composes? |
//! |---------|--------------|-----------|
//! | **Bindings** | Layered lookup (User > Policy > Base) | Yes |
//! | **Behavior** | `ModeKeyResolver` per mode | No (chosen) |
//!
//! `KeyLookupState` reports pure facts. The `ModeKeyResolver` applies policy
//! to decide what to do with those facts.
//!
//! # Example
//!
//! ```ignore
//! // Registry returns facts
//! let state = registry.query(&mode, &keys);
//!
//! // Vim resolver applies Vim policy
//! match state {
//!     KeyLookupState::ExactWithLonger { .. } => NeedMoreKeys,  // Wait for dd
//!     KeyLookupState::ExactOnly(cmd) => Execute(cmd),
//!     // ...
//! }
//!
//! // Eager resolver applies different policy
//! match state {
//!     KeyLookupState::ExactWithLonger { exact } => Execute(exact),  // Execute now!
//!     KeyLookupState::ExactOnly(cmd) => Execute(cmd),
//!     // ...
//! }
//! ```

use reovim_kernel::api::v1::{CommandId, ModeId};

use crate::KeySequence;

/// Pure mechanism: reports what exists in the registry.
///
/// This enum reports **FACTS** about what bindings exist for a given key sequence.
/// It does NOT make any policy decisions about what to do.
///
/// The `ModeKeyResolver` consumes these facts and applies its own policy
/// (e.g., Vim waits for longer sequences, Eager executes immediately).
///
/// # Variants
///
/// - `ExactOnly`: The key sequence matches exactly, with no longer bindings
/// - `ExactWithLonger`: The sequence matches AND longer bindings exist (e.g., `d` when `dd` exists)
/// - `PrefixOnly`: No exact match, but the sequence is a prefix of longer bindings
/// - `NotFound`: Nothing matches
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeyLookupState {
    /// Exact match exists, no longer bindings.
    ///
    /// Example: `x` maps to `delete_char`, and no `xx`, `xy`, etc. exist.
    ExactOnly(CommandId),

    /// Exact match exists AND longer bindings exist.
    ///
    /// Example: `d` maps to `enter_delete_op`, and `dd` maps to `delete_line`.
    /// The resolver must decide whether to execute `d` or wait for more keys.
    ExactWithLonger {
        /// The command for the exact match.
        exact: CommandId,
    },

    /// No exact match, but longer bindings exist.
    ///
    /// Example: `g` has no binding, but `gg` maps to `goto_top`.
    /// The resolver should wait for more keys.
    PrefixOnly,

    /// Nothing matches - no exact match, no longer bindings.
    ///
    /// The resolver should handle this (beep, ignore, or pass through).
    NotFound,
}

/// Binding layer for composition.
///
/// Bindings are organized into layers with priority ordering.
/// Higher layers override lower layers for the same key sequence.
///
/// # Layer Order (highest to lowest)
///
/// 1. **User** - User configuration overrides (`~/.config/reovim/keymap.toml`)
/// 2. **Policy** - Policy module defaults (Vim, Emacs, etc.)
/// 3. **Base** - Mechanism defaults (rarely used)
///
/// # Example
///
/// ```ignore
/// // Policy layer: d → delete
/// registry.register_at_layer(BindingLayer::Policy, &normal, keys("d"), cmd("delete"));
///
/// // User layer: d → custom_delete (overrides policy)
/// registry.register_at_layer(BindingLayer::User, &normal, keys("d"), cmd("custom_delete"));
///
/// // get_binding() returns custom_delete (User layer wins)
/// assert_eq!(registry.get_binding(&normal, &keys("d")), Some(cmd("custom_delete")));
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[repr(u8)]
pub enum BindingLayer {
    /// Base layer - mechanism defaults, rarely used.
    ///
    /// This is the lowest priority layer. Typically empty.
    Base = 0,

    /// Policy layer - policy module defaults.
    ///
    /// Vim module registers `hjkl`, `dd`, etc. here.
    /// Emacs module would register its bindings here.
    Policy = 1,

    /// User layer - user configuration overrides.
    ///
    /// Highest priority. User can override any binding.
    User = 2,
}

impl BindingLayer {
    /// Returns all layers in priority order (lowest to highest).
    #[must_use]
    pub const fn all() -> [Self; 3] {
        [Self::Base, Self::Policy, Self::User]
    }

    /// Returns the display name of this layer.
    #[must_use]
    pub const fn name(&self) -> &'static str {
        match self {
            Self::Base => "base",
            Self::Policy => "policy",
            Self::User => "user",
        }
    }
}

// ============================================================================
// KeyLookupPolicy Trait
// ============================================================================

/// Policy trait for interpreting key lookup results.
///
/// This trait defines how to interpret `KeyLookupState` facts. Different policies
/// make different decisions about what to do when multiple matches exist.
///
/// # Architecture
///
/// The `KeymapRegistry` provides FACTS via `query()`. The `KeyLookupPolicy`
/// interprets those facts to make a decision. This separation allows:
/// - Same registry + Vim policy → wait for longer sequences
/// - Same registry + Eager policy → execute exact matches immediately
///
/// # Example
///
/// ```ignore
/// use reovim_driver_input::{KeyLookupPolicy, KeyLookupState, KeyLookupResult};
///
/// struct VimLookupPolicy;
///
/// impl KeyLookupPolicy for VimLookupPolicy {
///     fn resolve(&self, state: KeyLookupState) -> KeyLookupResult {
///         match state {
///             // Vim: wait for longer sequences (dd might follow d)
///             KeyLookupState::ExactWithLonger { .. } => KeyLookupResult::Prefix,
///             KeyLookupState::ExactOnly(cmd) => KeyLookupResult::Found(cmd),
///             KeyLookupState::PrefixOnly => KeyLookupResult::Prefix,
///             KeyLookupState::NotFound => KeyLookupResult::NotFound,
///         }
///     }
/// }
/// ```
pub trait KeyLookupPolicy: Send + Sync {
    /// Interpret key lookup facts and return a result.
    ///
    /// # Arguments
    ///
    /// * `state` - The facts from `KeymapQuery::query()`
    ///
    /// # Returns
    ///
    /// A `KeyLookupResult` indicating what action to take.
    fn resolve(&self, state: KeyLookupState) -> KeyLookupResult;
}

/// Result of key lookup after policy is applied.
///
/// This enum represents the final decision after a `KeyLookupPolicy` has
/// interpreted the `KeyLookupState` facts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeyLookupResult {
    /// Execute this command.
    Found(CommandId),

    /// Wait for more keys (the sequence might become something else).
    Prefix,

    /// Nothing matches.
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

// ============================================================================
// Built-in Policies
// ============================================================================

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

/// Eager lookup policy: execute exact matches immediately.
///
/// When an exact match exists, execute it immediately regardless of whether
/// longer bindings exist. This is useful for editors that don't have Vim-style
/// operator-pending mode.
#[derive(Debug, Clone, Copy, Default)]
pub struct EagerLookupPolicy;

impl KeyLookupPolicy for EagerLookupPolicy {
    fn resolve(&self, state: KeyLookupState) -> KeyLookupResult {
        match state {
            // Eager: execute exact matches immediately
            KeyLookupState::ExactOnly(cmd) | KeyLookupState::ExactWithLonger { exact: cmd } => {
                KeyLookupResult::Found(cmd)
            }
            KeyLookupState::PrefixOnly => KeyLookupResult::Prefix,
            KeyLookupState::NotFound => KeyLookupResult::NotFound,
        }
    }
}

// ============================================================================
// KeymapQuery Trait
// ============================================================================

/// Trait for querying keybindings.
///
/// This trait provides the interface for resolvers to query the keymap registry
/// without depending directly on the runner. The runner implements this trait
/// for `KeymapRegistry`, allowing resolvers to get binding facts via `query()`.
///
/// # Architecture
///
/// This trait enables mechanism/policy separation:
/// - **Mechanism** (runner): Implements `KeymapQuery` for `KeymapRegistry`
/// - **Policy** (modules): Resolvers use `query()` to get facts, then apply policy
///
/// # Example
///
/// ```ignore
/// use reovim_driver_input::{KeymapQuery, KeyLookupState, KeySequence};
///
/// fn resolve_key(keymap: &dyn KeymapQuery, mode: &ModeId, keys: &KeySequence) {
///     match keymap.query(mode, keys) {
///         KeyLookupState::ExactWithLonger { exact } => {
///             // Vim policy: wait for more keys
///             // Eager policy: execute `exact` immediately
///         }
///         KeyLookupState::ExactOnly(cmd) => {
///             // Both policies: execute the command
///         }
///         KeyLookupState::PrefixOnly => {
///             // Both policies: wait for more keys
///         }
///         KeyLookupState::NotFound => {
///             // Handle unbound key
///         }
///     }
/// }
/// ```
pub trait KeymapQuery: Send + Sync {
    /// Query the keymap for a key sequence in a mode.
    ///
    /// Returns facts about what bindings exist - no policy decisions.
    /// The caller (resolver) decides what to do with the facts.
    fn query(&self, mode: &ModeId, keys: &KeySequence) -> KeyLookupState;

    /// Check if longer bindings exist for a key sequence.
    ///
    /// Convenience method - equivalent to checking if `query()` returns
    /// `ExactWithLonger` or `PrefixOnly`.
    fn has_longer_bindings(&self, mode: &ModeId, keys: &KeySequence) -> bool {
        matches!(
            self.query(mode, keys),
            KeyLookupState::ExactWithLonger { .. } | KeyLookupState::PrefixOnly
        )
    }

    /// Get the exact binding for a key sequence (if any).
    ///
    /// Convenience method - extracts the command from `ExactOnly` or `ExactWithLonger`.
    fn get_exact(&self, mode: &ModeId, keys: &KeySequence) -> Option<CommandId> {
        match self.query(mode, keys) {
            KeyLookupState::ExactOnly(cmd) | KeyLookupState::ExactWithLonger { exact: cmd } => {
                Some(cmd)
            }
            KeyLookupState::PrefixOnly | KeyLookupState::NotFound => None,
        }
    }

    /// Get all bindings that start with a given prefix.
    ///
    /// Returns bindings where the key sequence starts with `prefix` but is longer
    /// than `prefix`. This is used by which-key to show available completions.
    ///
    /// # Arguments
    ///
    /// * `mode` - The mode to query bindings for
    /// * `prefix` - The prefix to filter by
    ///
    /// # Returns
    ///
    /// A vector of (full key sequence, command ID) pairs for bindings that
    /// extend the given prefix. The caller can extract the suffix by skipping
    /// `prefix.len()` keys from the full sequence.
    ///
    /// # Default Implementation
    ///
    /// Returns an empty vector. Override in implementations that have
    /// access to the full binding registry.
    fn bindings_with_prefix(
        &self,
        _mode: &ModeId,
        _prefix: &KeySequence,
    ) -> Vec<(KeySequence, CommandId)> {
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use reovim_kernel::api::v1::ModuleId;

    use super::*;

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
    // VimLookupPolicy tests
    // ========================================================================

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
    fn test_key_lookup_policy_is_object_safe() {
        fn _accepts_ref(_: &dyn KeyLookupPolicy) {}
        fn _accepts_box(_: Box<dyn KeyLookupPolicy>) {}
    }

    // ========================================================================
    // KeymapQuery trait object safety
    // ========================================================================

    #[test]
    fn test_keymap_query_is_object_safe() {
        fn _accepts_ref(_: &dyn KeymapQuery) {}
        fn _accepts_box(_: Box<dyn KeymapQuery>) {}
    }
}
