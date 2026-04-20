//! Diagnostics panel mode key resolver.
//!
//! Routes key events in diagnostics panel mode:
//! - Bound keys (j/k, Enter, q, Esc, etc.) → `Execute` (keybinding lookup)
//! - All other keys → `NotHandled` (no character insertion)

use {
    reovim_driver_text_input::{
        KeyLookupState, KeySequence, ModeKeyResolver, ModeState, ResolveInput, ResolveResult,
    },
    reovim_kernel::api::v1::ModeId,
};

use crate::modes::DiagnosticsPanelMode;

/// Diagnostics panel mode key resolver.
///
/// All keys are delegated to keymap lookup. No character insertion
/// (unlike microscope which accepts query text).
pub struct DiagnosticsPanelResolver {
    mode_id: ModeId,
}

impl DiagnosticsPanelResolver {
    /// Create a new diagnostics panel resolver.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            mode_id: DiagnosticsPanelMode::PANEL_ID,
        }
    }
}

impl Default for DiagnosticsPanelResolver {
    fn default() -> Self {
        Self::new()
    }
}

impl ModeKeyResolver for DiagnosticsPanelResolver {
    fn resolve_with_keymap(
        &self,
        key: &reovim_driver_text_input::KeyEvent,
        _state: &mut ModeState,
        input: &ResolveInput<'_>,
    ) -> ResolveResult {
        let mut keys = KeySequence::new();
        keys.push(*key);
        let lookup_state = input.keymap.query(input.mode, &keys);

        match lookup_state {
            KeyLookupState::ExactWithLonger { exact, .. } | KeyLookupState::ExactOnly(exact) => {
                ResolveResult::Execute(exact, reovim_driver_text_input::ResolveContext::default())
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
