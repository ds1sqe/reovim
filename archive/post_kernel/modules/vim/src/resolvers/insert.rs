//! Vim insert mode key resolver.
//!
//! In insert mode, most keys insert characters directly. Special keys
//! like Escape exit insert mode, and control sequences trigger commands.

use {
    reovim_driver_input::{
        KeyCode, KeyEvent, KeyLookupState, KeySequence, ModeKeyResolver, ModeState, Modifiers,
        ResolveContext, ResolveInput, ResolveResult,
    },
    reovim_kernel::api::v1::ModeId,
};

use crate::modes::VimMode;

/// Vim insert mode key resolver.
///
/// Insert mode is primarily for text input:
/// - Most printable characters are inserted directly
/// - Escape exits to normal mode
/// - Some control sequences trigger commands (Ctrl+H for backspace, etc.)
/// - Arrow keys and special keys are handled via keymap lookup
///
/// # Example
///
/// ```ignore
/// let resolver = VimInsertResolver::new();
///
/// // Regular character - insert it
/// let result = resolver.resolve(&key('a'), &mut state);
/// assert!(matches!(result, ResolveResult::InsertChar('a')));
///
/// // Escape - exit to normal mode
/// let result = resolver.resolve(&KeyEvent::new(KeyCode::Escape), &mut state);
/// assert!(matches!(result, ResolveResult::ModeTransition(..)));
/// ```
pub struct VimInsertResolver {
    /// Mode ID for insert mode.
    mode_id: ModeId,
}

impl VimInsertResolver {
    /// Create a new insert mode resolver.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            mode_id: VimMode::INSERT_ID,
        }
    }

    /// Check if a key should insert a character.
    const fn is_insertable(key: &KeyEvent) -> Option<char> {
        // Only consider keys without control/alt modifiers for insertion
        // Shift is allowed (for uppercase letters)
        if key.modifiers.contains(Modifiers::CTRL) || key.modifiers.contains(Modifiers::ALT) {
            return None;
        }

        match key.code {
            KeyCode::Char(c) => Some(c),
            KeyCode::Tab => Some('\t'),
            KeyCode::Enter => Some('\n'),
            _ => None,
        }
    }
}

impl Default for VimInsertResolver {
    fn default() -> Self {
        Self::new()
    }
}

impl ModeKeyResolver for VimInsertResolver {
    /// Insert mode key resolution with keymap lookup.
    ///
    /// This method handles keymap lookup for non-insertable keys like Escape,
    /// Backspace, and arrow keys. When a key is not insertable, we query the
    /// keymap to find a bound command.
    ///
    /// # Architecture
    ///
    /// Insert mode differs from normal mode:
    /// - Insertable characters (letters, numbers, etc.) return `InsertChar`
    /// - Non-insertable keys (Escape, Backspace, arrows) query the keymap
    ///
    /// This enables Escape to trigger the `vim:exit-insert` command, which
    /// properly ends undo batching before switching to normal mode.
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

        // Non-insertable key - query keymap for binding
        // Use single-key lookup (insert mode has no multi-key sequences)
        let keys = KeySequence::from_keys(&[*key]);
        let lookup_state = input.keymap.query(input.mode, &keys);

        match lookup_state {
            KeyLookupState::ExactOnly(cmd) | KeyLookupState::ExactWithLonger { exact: cmd, .. } => {
                // Found a binding - execute it
                ResolveResult::Execute(cmd, ResolveContext::new())
            }
            KeyLookupState::PrefixOnly => {
                // Waiting for more keys (unlikely in insert mode)
                ResolveResult::Pending
            }
            KeyLookupState::NotFound => {
                // No binding - let runner handle (may be ignored)
                ResolveResult::NotHandled
            }
        }
    }

    fn mode_id(&self) -> &ModeId {
        &self.mode_id
    }

    fn inherits_from(&self) -> Option<&ModeId> {
        // Insert mode doesn't inherit from normal mode
        // (different key interpretation)
        None
    }

    fn reset(&mut self) {
        // No state to reset in insert mode
    }
}

#[cfg(test)]
mod tests {
    use {
        reovim_driver_input::{KeyLookupState, KeySequence, KeymapQuery},
        reovim_kernel::api::v1::ModeId,
    };

    use super::*;

    fn key(c: char) -> KeyEvent {
        KeyEvent::new(KeyCode::Char(c))
    }

    fn key_with_mod(c: char, modifiers: Modifiers) -> KeyEvent {
        KeyEvent::with_modifiers(KeyCode::Char(c), modifiers)
    }

    fn test_state() -> ModeState {
        ModeState::new(VimMode::INSERT_ID)
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
        static MODE: ModeId = VimMode::INSERT_ID;
        ResolveInput::new(&EMPTY_KEYS, &MODE, keymap)
    }

    #[test]
    fn test_new_resolver() {
        let resolver = VimInsertResolver::new();
        assert_eq!(resolver.mode_id(), &VimMode::INSERT_ID);
    }

    #[test]
    fn test_insert_character() {
        let resolver = VimInsertResolver::new();
        let mut state = test_state();
        let keymap = NotFoundKeymap;
        let input = resolve_input(&keymap);

        let result = resolver.resolve_with_keymap(&key('a'), &mut state, &input);
        assert!(matches!(result, ResolveResult::InsertChar('a')));

        let result = resolver.resolve_with_keymap(&key('Z'), &mut state, &input);
        assert!(matches!(result, ResolveResult::InsertChar('Z')));

        let result = resolver.resolve_with_keymap(&key('5'), &mut state, &input);
        assert!(matches!(result, ResolveResult::InsertChar('5')));

        let result = resolver.resolve_with_keymap(&key(' '), &mut state, &input);
        assert!(matches!(result, ResolveResult::InsertChar(' ')));
    }

    #[test]
    fn test_insert_tab() {
        let resolver = VimInsertResolver::new();
        let mut state = test_state();
        let keymap = NotFoundKeymap;
        let input = resolve_input(&keymap);

        let result = resolver.resolve_with_keymap(&KeyEvent::new(KeyCode::Tab), &mut state, &input);
        assert!(matches!(result, ResolveResult::InsertChar('\t')));
    }

    #[test]
    fn test_insert_enter() {
        let resolver = VimInsertResolver::new();
        let mut state = test_state();
        let keymap = NotFoundKeymap;
        let input = resolve_input(&keymap);

        let result =
            resolver.resolve_with_keymap(&KeyEvent::new(KeyCode::Enter), &mut state, &input);
        assert!(matches!(result, ResolveResult::InsertChar('\n')));
    }

    #[test]
    fn test_escape_not_handled() {
        // Escape is NOT handled by resolver - let keybinding call EXIT_INSERT command
        // This ensures undo batching is properly ended via the command.
        let resolver = VimInsertResolver::new();
        let mut state = test_state();
        let keymap = NotFoundKeymap;
        let input = resolve_input(&keymap);

        let result =
            resolver.resolve_with_keymap(&KeyEvent::new(KeyCode::Escape), &mut state, &input);
        assert!(matches!(result, ResolveResult::NotHandled));
    }

    #[test]
    fn test_ctrl_bracket_not_handled() {
        // Ctrl+[ is NOT handled by resolver - let keybinding call EXIT_INSERT command
        let resolver = VimInsertResolver::new();
        let mut state = test_state();
        let keymap = NotFoundKeymap;
        let input = resolve_input(&keymap);

        let result =
            resolver.resolve_with_keymap(&key_with_mod('[', Modifiers::CTRL), &mut state, &input);
        assert!(matches!(result, ResolveResult::NotHandled));
    }

    #[test]
    fn test_ctrl_char_not_inserted() {
        let resolver = VimInsertResolver::new();
        let mut state = test_state();
        let keymap = NotFoundKeymap;
        let input = resolve_input(&keymap);

        // Ctrl+H should NOT insert 'h', but go to keymap (for backspace behavior)
        let result =
            resolver.resolve_with_keymap(&key_with_mod('h', Modifiers::CTRL), &mut state, &input);
        assert!(matches!(result, ResolveResult::NotHandled));
    }

    #[test]
    fn test_alt_char_not_inserted() {
        let resolver = VimInsertResolver::new();
        let mut state = test_state();
        let keymap = NotFoundKeymap;
        let input = resolve_input(&keymap);

        // Alt+a should NOT insert 'a'
        let result =
            resolver.resolve_with_keymap(&key_with_mod('a', Modifiers::ALT), &mut state, &input);
        assert!(matches!(result, ResolveResult::NotHandled));
    }

    #[test]
    fn test_backspace_not_handled() {
        let resolver = VimInsertResolver::new();
        let mut state = test_state();
        let keymap = NotFoundKeymap;
        let input = resolve_input(&keymap);

        // Backspace goes to keymap lookup
        let result =
            resolver.resolve_with_keymap(&KeyEvent::new(KeyCode::Backspace), &mut state, &input);
        assert!(matches!(result, ResolveResult::NotHandled));
    }

    #[test]
    fn test_arrow_keys_not_handled() {
        let resolver = VimInsertResolver::new();
        let mut state = test_state();
        let keymap = NotFoundKeymap;
        let input = resolve_input(&keymap);

        let result =
            resolver.resolve_with_keymap(&KeyEvent::new(KeyCode::Left), &mut state, &input);
        assert!(matches!(result, ResolveResult::NotHandled));

        let result = resolver.resolve_with_keymap(&KeyEvent::new(KeyCode::Up), &mut state, &input);
        assert!(matches!(result, ResolveResult::NotHandled));
    }

    #[test]
    fn test_mode_id() {
        let resolver = VimInsertResolver::new();
        assert_eq!(resolver.mode_id().name(), "insert");
    }

    #[test]
    fn test_inherits_from() {
        let resolver = VimInsertResolver::new();
        assert!(resolver.inherits_from().is_none());
    }

    #[test]
    fn test_shift_allowed_for_uppercase() {
        let resolver = VimInsertResolver::new();
        let mut state = test_state();
        let keymap = NotFoundKeymap;
        let input = resolve_input(&keymap);

        // Shift+a (uppercase A) should insert
        let result =
            resolver.resolve_with_keymap(&key_with_mod('A', Modifiers::SHIFT), &mut state, &input);
        assert!(matches!(result, ResolveResult::InsertChar('A')));
    }
}
