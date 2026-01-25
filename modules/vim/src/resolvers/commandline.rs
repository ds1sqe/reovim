//! Vim command-line mode key resolver.
//!
//! In command-line mode (`:`, `/`, `?`), typed characters accumulate in
//! the command-line buffer. Enter executes, Escape cancels.

use {
    reovim_driver_input::{
        KeyCode, KeyEvent, KeyLookupState, KeySequence, ModeKeyResolver, ModeState, Modifiers,
        ResolveContext, ResolveInput, ResolveResult,
    },
    reovim_kernel::api::v1::ModeId,
};

use crate::modes::VimMode;

/// Vim command-line mode key resolver.
///
/// Handles input for `:` (Ex commands), `/` (forward search), and `?` (backward search).
/// Characters are accumulated in the command-line buffer until Enter or Escape.
///
/// # Behavior
///
/// - Printable characters → `InsertChar` (goes to cmdline buffer)
/// - Enter → `NotHandled` (keybinding executes command/search)
/// - Escape → `NotHandled` (keybinding cancels)
/// - Backspace → `NotHandled` (keybinding deletes char)
pub struct VimCommandLineResolver {
    mode_id: ModeId,
}

impl VimCommandLineResolver {
    /// Create a new command-line mode resolver.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            mode_id: VimMode::COMMANDLINE_ID,
        }
    }

    /// Check if a key should insert a character into the command-line buffer.
    const fn is_insertable(key: &KeyEvent) -> Option<char> {
        // Only consider keys without control/alt modifiers for insertion
        if key.modifiers.contains(Modifiers::CTRL) || key.modifiers.contains(Modifiers::ALT) {
            return None;
        }

        match key.code {
            KeyCode::Char(c) => Some(c),
            // Space is insertable
            // Tab could be for completion (future)
            // Enter is NOT insertable - it executes
            _ => None,
        }
    }
}

impl Default for VimCommandLineResolver {
    fn default() -> Self {
        Self::new()
    }
}

impl ModeKeyResolver for VimCommandLineResolver {
    fn resolve_with_keymap(
        &self,
        key: &KeyEvent,
        _state: &mut ModeState,
        input: &ResolveInput<'_>,
    ) -> ResolveResult {
        // Check for insertable character first
        if let Some(c) = Self::is_insertable(key) {
            return ResolveResult::InsertChar(c);
        }

        // For non-insertable keys (Escape, Enter, Backspace, etc.), look up in keymap
        let mut keys = KeySequence::new();
        keys.push(*key);
        let lookup_state = input.keymap.query(input.mode, &keys);

        match lookup_state {
            KeyLookupState::ExactWithLonger { exact, .. } | KeyLookupState::ExactOnly(exact) => {
                // Execute the command
                ResolveResult::Execute(exact, ResolveContext::default())
            }
            KeyLookupState::PrefixOnly => {
                // Wait for more keys
                ResolveResult::Pending
            }
            KeyLookupState::NotFound => {
                // No binding found
                ResolveResult::NotHandled
            }
        }
    }

    fn mode_id(&self) -> &ModeId {
        &self.mode_id
    }

    fn inherits_from(&self) -> Option<&ModeId> {
        // Command-line mode doesn't inherit from other modes
        None
    }

    fn reset(&mut self) {
        // No state to reset
    }
}

#[cfg(test)]
mod tests {
    use reovim_driver_input::KeymapQuery;

    use super::*;

    fn key(c: char) -> KeyEvent {
        KeyEvent::new(KeyCode::Char(c))
    }

    fn key_with_mod(c: char, modifiers: Modifiers) -> KeyEvent {
        KeyEvent::with_modifiers(KeyCode::Char(c), modifiers)
    }

    fn test_state() -> ModeState {
        ModeState::new(VimMode::COMMANDLINE_ID)
    }

    /// Mock keymap that always returns NotFound (no bindings).
    struct NotFoundKeymap;

    impl KeymapQuery for NotFoundKeymap {
        fn query(&self, _mode: &ModeId, _keys: &KeySequence) -> KeyLookupState {
            KeyLookupState::NotFound
        }
    }

    fn resolve_input(keymap: &impl KeymapQuery) -> ResolveInput<'_> {
        static EMPTY_KEYS: KeySequence = KeySequence::new();
        static MODE: ModeId = VimMode::COMMANDLINE_ID;
        ResolveInput::new(&EMPTY_KEYS, &MODE, keymap)
    }

    #[test]
    fn test_new_resolver() {
        let resolver = VimCommandLineResolver::new();
        assert_eq!(resolver.mode_id(), &VimMode::COMMANDLINE_ID);
    }

    #[test]
    fn test_insert_character() {
        let resolver = VimCommandLineResolver::new();
        let mut state = test_state();
        let keymap = NotFoundKeymap;
        let input = resolve_input(&keymap);

        let result = resolver.resolve_with_keymap(&key('w'), &mut state, &input);
        assert!(matches!(result, ResolveResult::InsertChar('w')));

        let result = resolver.resolve_with_keymap(&key('q'), &mut state, &input);
        assert!(matches!(result, ResolveResult::InsertChar('q')));

        let result = resolver.resolve_with_keymap(&key(' '), &mut state, &input);
        assert!(matches!(result, ResolveResult::InsertChar(' ')));
    }

    #[test]
    fn test_escape_not_handled() {
        let resolver = VimCommandLineResolver::new();
        let mut state = test_state();
        let keymap = NotFoundKeymap;
        let input = resolve_input(&keymap);

        let result =
            resolver.resolve_with_keymap(&KeyEvent::new(KeyCode::Escape), &mut state, &input);
        assert!(matches!(result, ResolveResult::NotHandled));
    }

    #[test]
    fn test_enter_not_handled() {
        let resolver = VimCommandLineResolver::new();
        let mut state = test_state();
        let keymap = NotFoundKeymap;
        let input = resolve_input(&keymap);

        let result =
            resolver.resolve_with_keymap(&KeyEvent::new(KeyCode::Enter), &mut state, &input);
        assert!(matches!(result, ResolveResult::NotHandled));
    }

    #[test]
    fn test_backspace_not_handled() {
        let resolver = VimCommandLineResolver::new();
        let mut state = test_state();
        let keymap = NotFoundKeymap;
        let input = resolve_input(&keymap);

        let result =
            resolver.resolve_with_keymap(&KeyEvent::new(KeyCode::Backspace), &mut state, &input);
        assert!(matches!(result, ResolveResult::NotHandled));
    }

    #[test]
    fn test_ctrl_char_not_inserted() {
        let resolver = VimCommandLineResolver::new();
        let mut state = test_state();
        let keymap = NotFoundKeymap;
        let input = resolve_input(&keymap);

        let result =
            resolver.resolve_with_keymap(&key_with_mod('c', Modifiers::CTRL), &mut state, &input);
        assert!(matches!(result, ResolveResult::NotHandled));
    }

    #[test]
    fn test_mode_id() {
        let resolver = VimCommandLineResolver::new();
        assert_eq!(resolver.mode_id().name(), "command");
    }

    #[test]
    fn test_inherits_from() {
        let resolver = VimCommandLineResolver::new();
        assert!(resolver.inherits_from().is_none());
    }
}
