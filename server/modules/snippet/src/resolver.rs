//! Snippet mode key resolver (#136).
//!
//! `SnippetResolver` is the `ModeKeyResolver` for the `snippet:navigating` mode.
//! It handles snippet-specific key bindings (Tab, S-Tab, Esc) and delegates
//! unhandled keys to the vim insert resolver via `inherits_from()`.

use {
    reovim_driver_input::{
        KeyEvent, KeyLookupState, KeySequence, ModeKeyResolver, ModeState, ResolveContext,
        ResolveInput, ResolveResult,
    },
    reovim_kernel::api::v1::ModeId,
};

use crate::ids;

/// Key resolver for snippet navigation mode.
///
/// When the snippet module activates `snippet:navigating` mode, this
/// resolver handles Tab/S-Tab for tab stop navigation and Esc for
/// cancellation. Unhandled keys fall through to vim insert mode via
/// the `inherits_from()` chain in `ResolverRegistry`.
pub struct SnippetResolver {
    /// The mode ID for this resolver (owned to satisfy lifetime requirements).
    mode_id: ModeId,
    /// The parent mode ID for inheritance (owned).
    parent_mode_id: ModeId,
}

impl SnippetResolver {
    /// Create a new snippet resolver.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            mode_id: ids::NAVIGATING_MODE,
            parent_mode_id: ids::VIM_INSERT_MODE,
        }
    }
}

impl Default for SnippetResolver {
    fn default() -> Self {
        Self::new()
    }
}

impl ModeKeyResolver for SnippetResolver {
    fn mode_id(&self) -> &ModeId {
        &self.mode_id
    }

    fn inherits_from(&self) -> Option<&ModeId> {
        // Fall through to vim insert for unhandled keys
        Some(&self.parent_mode_id)
    }

    fn resolve_with_keymap(
        &self,
        key: &KeyEvent,
        _state: &mut ModeState,
        input: &ResolveInput<'_>,
    ) -> ResolveResult {
        // Query keymap for snippet-specific bindings
        let keys = KeySequence::from_keys(&[*key]);
        match input.keymap.query(self.mode_id(), &keys) {
            KeyLookupState::ExactOnly(cmd) | KeyLookupState::ExactWithLonger { exact: cmd } => {
                ResolveResult::Execute(cmd, ResolveContext::new())
            }
            KeyLookupState::PrefixOnly => ResolveResult::Pending,
            KeyLookupState::NotFound => ResolveResult::NotHandled,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mode_id() {
        let resolver = SnippetResolver::new();
        assert_eq!(resolver.mode_id(), &ids::NAVIGATING_MODE);
    }

    #[test]
    fn test_inherits_from_vim_insert() {
        let resolver = SnippetResolver::new();
        let parent = resolver.inherits_from().unwrap();
        assert_eq!(parent, &ids::VIM_INSERT_MODE);
    }

    #[test]
    fn test_inherits_from_is_some() {
        let resolver = SnippetResolver::new();
        assert!(resolver.inherits_from().is_some());
    }

    #[test]
    fn test_default() {
        let resolver = SnippetResolver::default();
        assert_eq!(resolver.mode_id(), &ids::NAVIGATING_MODE);
    }
}
