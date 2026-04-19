use reovim_kernel::api::v1::{CommandId, ModeId};

use crate::{BindingInfo, KeySequence};

/// Pure mechanism facts about what bindings exist for a sequence.
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

/// Binding layer for composition.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[repr(u8)]
pub enum BindingLayer {
    /// Base layer.
    Base = 0,
    /// Policy defaults.
    Policy = 1,
    /// User overrides.
    User = 2,
}

impl BindingLayer {
    /// Return all layers in priority order.
    #[must_use]
    pub const fn all() -> [Self; 3] {
        [Self::Base, Self::Policy, Self::User]
    }

    /// Return the display name of this layer.
    #[must_use]
    pub const fn name(&self) -> &'static str {
        match self {
            Self::Base => "base",
            Self::Policy => "policy",
            Self::User => "user",
        }
    }
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

/// Trait for querying keybindings.
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

#[cfg(test)]
mod tests {
    use {super::*, reovim_kernel::api::v1::ModuleId};

    fn test_module() -> ModuleId {
        ModuleId::new("test")
    }

    fn test_command(name: &'static str) -> CommandId {
        CommandId::new(test_module(), name)
    }

    fn test_mode(name: &'static str) -> ModeId {
        ModeId::new(test_module(), name)
    }

    struct MockKeymap {
        state: KeyLookupState,
    }

    impl KeymapQuery for MockKeymap {
        fn query(&self, _mode: &ModeId, _keys: &KeySequence) -> KeyLookupState {
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
}
