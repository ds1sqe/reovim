//! Module manager mode key resolver (#622).
//!
//! Routes key events in module manager mode via keymap lookup.
//! No character input - all keys go through the keybinding system.

use {
    reovim_driver_text_input::{
        KeyEvent, KeyLookupState, KeySequence, ModeKeyResolver, ModeState, ResolveContext,
        ResolveInput, ResolveResult,
    },
    reovim_kernel::api::v1::ModeId,
};

use crate::modes::ManagerMode;

/// Module manager mode key resolver.
///
/// All keys are dispatched through the keymap. No character insertion.
pub struct ManagerResolver {
    mode_id: ModeId,
}

impl ManagerResolver {
    /// Create a new manager resolver.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            mode_id: ManagerMode::MANAGER_ID,
        }
    }
}

impl Default for ManagerResolver {
    fn default() -> Self {
        Self::new()
    }
}

impl ModeKeyResolver for ManagerResolver {
    fn resolve_with_keymap(
        &self,
        key: &KeyEvent,
        _state: &mut ModeState,
        input: &ResolveInput<'_>,
    ) -> ResolveResult {
        let mut keys = KeySequence::new();
        keys.push(*key);
        let lookup_state = input.keymap.query(input.mode, &keys);

        match lookup_state {
            KeyLookupState::ExactWithLonger { exact, .. } | KeyLookupState::ExactOnly(exact) => {
                ResolveResult::Execute(exact, ResolveContext::default())
            }
            KeyLookupState::PrefixOnly => ResolveResult::Pending,
            KeyLookupState::NotFound => ResolveResult::NotHandled,
        }
    }

    fn mode_id(&self) -> &ModeId {
        &self.mode_id
    }

    fn inherits_from(&self) -> Option<&ModeId> {
        None
    }

    fn reset(&mut self) {
        // No state to reset.
    }
}

#[cfg(test)]
#[path = "resolver_tests.rs"]
mod tests;
