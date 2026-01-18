//! Vim insert mode key resolver.
//!
//! In insert mode, most keys insert characters directly. Special keys
//! like Escape exit insert mode, and control sequences trigger commands.

use {
    reovim_driver_input::{
        KeyCode, KeyEvent, ModeKeyResolver, ModeState, ModeTransition, Modifiers, ResolveResult,
        TransitionContext,
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

    /// Check if this is an escape key to exit insert mode.
    fn is_escape(key: &KeyEvent) -> bool {
        // Escape or Ctrl+[ both exit insert mode
        key.code == KeyCode::Escape
            || (key.code == KeyCode::Char('[') && key.modifiers.contains(Modifiers::CTRL))
    }
}

impl Default for VimInsertResolver {
    fn default() -> Self {
        Self::new()
    }
}

impl ModeKeyResolver for VimInsertResolver {
    fn resolve(&self, key: &KeyEvent, _state: &mut ModeState) -> ResolveResult {
        // Escape exits insert mode
        if Self::is_escape(key) {
            return ResolveResult::ModeTransition(ModeTransition::Set {
                mode: VimMode::NORMAL_ID,
                context: TransitionContext::new(),
            });
        }

        // Check for insertable character
        if let Some(c) = Self::is_insertable(key) {
            return ResolveResult::InsertChar(c);
        }

        // Other keys (Backspace, arrows, Ctrl+sequences) go to keymap lookup
        // Return NotHandled to let the runner do the lookup
        ResolveResult::NotHandled
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

    #[test]
    fn test_new_resolver() {
        let resolver = VimInsertResolver::new();
        assert_eq!(resolver.mode_id(), &VimMode::INSERT_ID);
    }

    #[test]
    fn test_insert_character() {
        let resolver = VimInsertResolver::new();
        let mut state = test_state();

        let result = resolver.resolve(&key('a'), &mut state);
        assert!(matches!(result, ResolveResult::InsertChar('a')));

        let result = resolver.resolve(&key('Z'), &mut state);
        assert!(matches!(result, ResolveResult::InsertChar('Z')));

        let result = resolver.resolve(&key('5'), &mut state);
        assert!(matches!(result, ResolveResult::InsertChar('5')));

        let result = resolver.resolve(&key(' '), &mut state);
        assert!(matches!(result, ResolveResult::InsertChar(' ')));
    }

    #[test]
    fn test_insert_tab() {
        let resolver = VimInsertResolver::new();
        let mut state = test_state();

        let result = resolver.resolve(&KeyEvent::new(KeyCode::Tab), &mut state);
        assert!(matches!(result, ResolveResult::InsertChar('\t')));
    }

    #[test]
    fn test_insert_enter() {
        let resolver = VimInsertResolver::new();
        let mut state = test_state();

        let result = resolver.resolve(&KeyEvent::new(KeyCode::Enter), &mut state);
        assert!(matches!(result, ResolveResult::InsertChar('\n')));
    }

    #[test]
    fn test_escape_exits() {
        let resolver = VimInsertResolver::new();
        let mut state = test_state();

        let result = resolver.resolve(&KeyEvent::new(KeyCode::Escape), &mut state);

        if let ResolveResult::ModeTransition(ModeTransition::Set { mode, .. }) = result {
            assert_eq!(mode, VimMode::NORMAL_ID);
        } else {
            panic!("expected ModeTransition::Set");
        }
    }

    #[test]
    fn test_ctrl_bracket_exits() {
        let resolver = VimInsertResolver::new();
        let mut state = test_state();

        let result = resolver.resolve(&key_with_mod('[', Modifiers::CTRL), &mut state);

        if let ResolveResult::ModeTransition(ModeTransition::Set { mode, .. }) = result {
            assert_eq!(mode, VimMode::NORMAL_ID);
        } else {
            panic!("expected ModeTransition::Set for Ctrl+[");
        }
    }

    #[test]
    fn test_ctrl_char_not_inserted() {
        let resolver = VimInsertResolver::new();
        let mut state = test_state();

        // Ctrl+H should NOT insert 'h', but go to keymap (for backspace behavior)
        let result = resolver.resolve(&key_with_mod('h', Modifiers::CTRL), &mut state);
        assert!(matches!(result, ResolveResult::NotHandled));
    }

    #[test]
    fn test_alt_char_not_inserted() {
        let resolver = VimInsertResolver::new();
        let mut state = test_state();

        // Alt+a should NOT insert 'a'
        let result = resolver.resolve(&key_with_mod('a', Modifiers::ALT), &mut state);
        assert!(matches!(result, ResolveResult::NotHandled));
    }

    #[test]
    fn test_backspace_not_handled() {
        let resolver = VimInsertResolver::new();
        let mut state = test_state();

        // Backspace goes to keymap lookup
        let result = resolver.resolve(&KeyEvent::new(KeyCode::Backspace), &mut state);
        assert!(matches!(result, ResolveResult::NotHandled));
    }

    #[test]
    fn test_arrow_keys_not_handled() {
        let resolver = VimInsertResolver::new();
        let mut state = test_state();

        let result = resolver.resolve(&KeyEvent::new(KeyCode::Left), &mut state);
        assert!(matches!(result, ResolveResult::NotHandled));

        let result = resolver.resolve(&KeyEvent::new(KeyCode::Up), &mut state);
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

        // Shift+a (uppercase A) should insert
        let result = resolver.resolve(&key_with_mod('A', Modifiers::SHIFT), &mut state);
        assert!(matches!(result, ResolveResult::InsertChar('A')));
    }
}
