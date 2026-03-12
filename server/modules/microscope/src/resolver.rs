//! Microscope mode key resolver.
//!
//! Routes key events in microscope picker mode:
//! - Printable characters → `InsertChar` (query field via `MicroscopeState`)
//! - Bound keys (Esc, BS, CR, C-n, C-p, etc.) → `Execute` (keybinding lookup)
//! - Unbound special keys → `NotHandled`

use {
    reovim_driver_input::{
        KeyCode, KeyEvent, KeyLookupState, KeySequence, ModeKeyResolver, ModeState, Modifiers,
        ResolveInput, ResolveResult,
    },
    reovim_kernel::api::v1::ModeId,
};

use crate::{modes::MicroscopeMode, state::MicroscopeState};

/// Microscope picker mode key resolver.
///
/// Follows the same pattern as `VimCommandLineResolver`:
/// 1. Printable characters → `InsertChar` targeting `MicroscopeState`
/// 2. Special keys → keymap lookup for registered bindings
pub struct MicroscopeResolver {
    mode_id: ModeId,
}

impl MicroscopeResolver {
    /// Create a new microscope resolver.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            mode_id: MicroscopeMode::PICKER_ID,
        }
    }

    /// Check if a key should insert a character into the query.
    const fn is_insertable(key: &KeyEvent) -> Option<char> {
        if key.modifiers.contains(Modifiers::CTRL) || key.modifiers.contains(Modifiers::ALT) {
            return None;
        }

        match key.code {
            KeyCode::Char(c) => Some(c),
            _ => None,
        }
    }
}

impl Default for MicroscopeResolver {
    fn default() -> Self {
        Self::new()
    }
}

impl ModeKeyResolver for MicroscopeResolver {
    fn resolve_with_keymap(
        &self,
        key: &KeyEvent,
        _state: &mut ModeState,
        input: &ResolveInput<'_>,
    ) -> ResolveResult {
        // Printable characters go to the query field.
        if let Some(c) = Self::is_insertable(key) {
            return ResolveResult::insert_char_to::<MicroscopeState>(c);
        }

        // Special keys: look up in keymap.
        let mut keys = KeySequence::new();
        keys.push(*key);
        let lookup_state = input.keymap.query(input.mode, &keys);

        match lookup_state {
            KeyLookupState::ExactWithLonger { exact, .. } | KeyLookupState::ExactOnly(exact) => {
                ResolveResult::Execute(exact, reovim_driver_input::ResolveContext::default())
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
