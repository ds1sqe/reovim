//! Vim command-line mode key resolver.
//!
//! In command-line mode (`:`, `/`, `?`), typed characters accumulate in
//! the command-line buffer. Enter executes, Escape cancels.

use {
    reovim_driver_input::{
        KeyCode, KeyEvent, KeyLookupState, KeySequence, ModeKeyResolver, ModeState, Modifiers,
        ResolveContext, ResolveInput, ResolveResult,
    },
    reovim_driver_session::CmdlineState,
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
        // Route to CmdlineState extension (#482 - Generic Input Target)
        if let Some(c) = Self::is_insertable(key) {
            return ResolveResult::insert_char_to::<CmdlineState>(c);
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
#[allow(clippy::doc_markdown)]
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

    /// Mock keymap that always returns `NotFound` (no bindings).
    struct NotFoundKeymap;

    #[cfg_attr(coverage_nightly, coverage(off))]
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

        // Command-line mode routes to CmdlineState extension (InputTarget::Extension)
        let result = resolver.resolve_with_keymap(&key('w'), &mut state, &input);
        assert!(matches!(result, ResolveResult::InsertChar { char: 'w', .. }));

        let result = resolver.resolve_with_keymap(&key('q'), &mut state, &input);
        assert!(matches!(result, ResolveResult::InsertChar { char: 'q', .. }));

        let result = resolver.resolve_with_keymap(&key(' '), &mut state, &input);
        assert!(matches!(result, ResolveResult::InsertChar { char: ' ', .. }));
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

    // ========================================================================
    // Additional Command-Line Resolver Tests
    // ========================================================================

    #[test]
    fn test_default_impl() {
        let resolver = VimCommandLineResolver::default();
        assert_eq!(resolver.mode_id(), &VimMode::COMMANDLINE_ID);
    }

    #[test]
    fn test_reset_is_noop() {
        let mut resolver = VimCommandLineResolver::new();
        resolver.reset();
        assert_eq!(resolver.mode_id(), &VimMode::COMMANDLINE_ID);
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_insert_digits() {
        let resolver = VimCommandLineResolver::new();
        let mut state = test_state();
        let keymap = NotFoundKeymap;
        let input = resolve_input(&keymap);

        for c in '0'..='9' {
            let result = resolver.resolve_with_keymap(&key(c), &mut state, &input);
            assert!(
                matches!(result, ResolveResult::InsertChar { char: ch, .. } if ch == c),
                "digit '{c}' should be insertable"
            );
        }
    }

    #[test]
    fn test_insert_special_symbols() {
        let resolver = VimCommandLineResolver::new();
        let mut state = test_state();
        let keymap = NotFoundKeymap;
        let input = resolve_input(&keymap);

        let result = resolver.resolve_with_keymap(&key('/'), &mut state, &input);
        assert!(matches!(result, ResolveResult::InsertChar { char: '/', .. }));

        let result = resolver.resolve_with_keymap(&key('!'), &mut state, &input);
        assert!(matches!(result, ResolveResult::InsertChar { char: '!', .. }));

        let result = resolver.resolve_with_keymap(&key('.'), &mut state, &input);
        assert!(matches!(result, ResolveResult::InsertChar { char: '.', .. }));
    }

    #[test]
    fn test_alt_char_not_inserted() {
        let resolver = VimCommandLineResolver::new();
        let mut state = test_state();
        let keymap = NotFoundKeymap;
        let input = resolve_input(&keymap);

        let result =
            resolver.resolve_with_keymap(&key_with_mod('a', Modifiers::ALT), &mut state, &input);
        assert!(matches!(result, ResolveResult::NotHandled));
    }

    #[test]
    fn test_tab_not_inserted() {
        // Tab is not insertable in command-line mode (might be for completion)
        let resolver = VimCommandLineResolver::new();
        let mut state = test_state();
        let keymap = NotFoundKeymap;
        let input = resolve_input(&keymap);

        let result = resolver.resolve_with_keymap(&KeyEvent::new(KeyCode::Tab), &mut state, &input);
        assert!(matches!(result, ResolveResult::NotHandled));
    }

    #[test]
    fn test_arrow_keys_not_handled() {
        let resolver = VimCommandLineResolver::new();
        let mut state = test_state();
        let keymap = NotFoundKeymap;
        let input = resolve_input(&keymap);

        let result = resolver.resolve_with_keymap(&KeyEvent::new(KeyCode::Up), &mut state, &input);
        assert!(matches!(result, ResolveResult::NotHandled));

        let result =
            resolver.resolve_with_keymap(&KeyEvent::new(KeyCode::Down), &mut state, &input);
        assert!(matches!(result, ResolveResult::NotHandled));
    }

    /// Mock keymap returning ExactOnly for testing keymap interaction.
    struct ExactKeymap;

    #[cfg_attr(coverage_nightly, coverage(off))]
    impl KeymapQuery for ExactKeymap {
        fn query(&self, _mode: &ModeId, _keys: &KeySequence) -> KeyLookupState {
            KeyLookupState::ExactOnly(reovim_kernel::api::v1::CommandId::new(
                reovim_kernel::api::v1::ModuleId::new("test"),
                "test-cmd",
            ))
        }
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_escape_executes_with_keymap_binding() {
        let resolver = VimCommandLineResolver::new();
        let mut state = test_state();
        let keymap = ExactKeymap;
        let input = resolve_input(&keymap);

        let result =
            resolver.resolve_with_keymap(&KeyEvent::new(KeyCode::Escape), &mut state, &input);
        match result {
            ResolveResult::Execute(cmd, _) => {
                assert_eq!(cmd.name(), "test-cmd");
            }
            _ => panic!("expected Execute, got {result:?}"),
        }
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_enter_executes_with_keymap_binding() {
        let resolver = VimCommandLineResolver::new();
        let mut state = test_state();
        let keymap = ExactKeymap;
        let input = resolve_input(&keymap);

        let result =
            resolver.resolve_with_keymap(&KeyEvent::new(KeyCode::Enter), &mut state, &input);
        match result {
            ResolveResult::Execute(cmd, _) => {
                assert_eq!(cmd.name(), "test-cmd");
            }
            _ => panic!("expected Execute, got {result:?}"),
        }
    }

    /// Mock keymap returning PrefixOnly for testing.
    struct PrefixKeymap;

    #[cfg_attr(coverage_nightly, coverage(off))]
    impl KeymapQuery for PrefixKeymap {
        fn query(&self, _mode: &ModeId, _keys: &KeySequence) -> KeyLookupState {
            KeyLookupState::PrefixOnly
        }
    }

    #[test]
    fn test_prefix_only_returns_pending() {
        let resolver = VimCommandLineResolver::new();
        let mut state = test_state();
        let keymap = PrefixKeymap;
        let input = resolve_input(&keymap);

        let result =
            resolver.resolve_with_keymap(&KeyEvent::new(KeyCode::Escape), &mut state, &input);
        assert!(matches!(result, ResolveResult::Pending));
    }

    #[test]
    fn test_delete_key_not_handled() {
        let resolver = VimCommandLineResolver::new();
        let mut state = test_state();
        let keymap = NotFoundKeymap;
        let input = resolve_input(&keymap);

        let result =
            resolver.resolve_with_keymap(&KeyEvent::new(KeyCode::Delete), &mut state, &input);
        assert!(matches!(result, ResolveResult::NotHandled));
    }

    #[test]
    fn test_mode_id_module() {
        let resolver = VimCommandLineResolver::new();
        assert_eq!(resolver.mode_id().module().as_str(), "vim");
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_insert_uppercase_chars() {
        let resolver = VimCommandLineResolver::new();
        let mut state = test_state();
        let keymap = NotFoundKeymap;
        let input = resolve_input(&keymap);

        for c in 'A'..='Z' {
            let result = resolver.resolve_with_keymap(&key(c), &mut state, &input);
            assert!(
                matches!(result, ResolveResult::InsertChar { char: ch, .. } if ch == c),
                "uppercase '{c}' should be insertable"
            );
        }
    }

    #[test]
    fn test_insert_unicode_char() {
        let resolver = VimCommandLineResolver::new();
        let mut state = test_state();
        let keymap = NotFoundKeymap;
        let input = resolve_input(&keymap);

        let result = resolver.resolve_with_keymap(
            &KeyEvent::new(KeyCode::Char('\u{00e9}')),
            &mut state,
            &input,
        );
        assert!(matches!(
            result,
            ResolveResult::InsertChar {
                char: '\u{00e9}',
                ..
            }
        ));
    }

    #[test]
    fn test_ctrl_shift_not_inserted() {
        let resolver = VimCommandLineResolver::new();
        let mut state = test_state();
        let keymap = NotFoundKeymap;
        let input = resolve_input(&keymap);

        // Ctrl+Shift combination should not insert
        let result = resolver.resolve_with_keymap(
            &key_with_mod('a', Modifiers::CTRL | Modifiers::SHIFT),
            &mut state,
            &input,
        );
        assert!(matches!(result, ResolveResult::NotHandled));
    }

    #[test]
    fn test_home_key_not_handled() {
        let resolver = VimCommandLineResolver::new();
        let mut state = test_state();
        let keymap = NotFoundKeymap;
        let input = resolve_input(&keymap);

        let result =
            resolver.resolve_with_keymap(&KeyEvent::new(KeyCode::Home), &mut state, &input);
        assert!(matches!(result, ResolveResult::NotHandled));
    }

    #[test]
    fn test_end_key_not_handled() {
        let resolver = VimCommandLineResolver::new();
        let mut state = test_state();
        let keymap = NotFoundKeymap;
        let input = resolve_input(&keymap);

        let result = resolver.resolve_with_keymap(&KeyEvent::new(KeyCode::End), &mut state, &input);
        assert!(matches!(result, ResolveResult::NotHandled));
    }

    /// Mock keymap returning `ExactWithLonger`.
    struct ExactWithLongerKeymap;

    #[cfg_attr(coverage_nightly, coverage(off))]
    impl KeymapQuery for ExactWithLongerKeymap {
        fn query(&self, _mode: &ModeId, _keys: &KeySequence) -> KeyLookupState {
            KeyLookupState::ExactWithLonger {
                exact: reovim_kernel::api::v1::CommandId::new(
                    reovim_kernel::api::v1::ModuleId::new("test"),
                    "test-cmd",
                ),
            }
        }
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_exact_with_longer_executes() {
        let resolver = VimCommandLineResolver::new();
        let mut state = test_state();
        let keymap = ExactWithLongerKeymap;
        let input = resolve_input(&keymap);

        let result =
            resolver.resolve_with_keymap(&KeyEvent::new(KeyCode::Enter), &mut state, &input);
        match result {
            ResolveResult::Execute(cmd, _) => {
                assert_eq!(cmd.name(), "test-cmd");
            }
            _ => panic!("expected Execute, got {result:?}"),
        }
    }

    #[test]
    fn test_const_new() {
        const RESOLVER: VimCommandLineResolver = VimCommandLineResolver::new();
        assert_eq!(RESOLVER.mode_id().name(), "command");
    }

    #[test]
    fn test_shift_char_is_insertable() {
        // Shift+char (like uppercase) should still be insertable
        // because the resulting KeyCode is Char with the uppercase char
        let resolver = VimCommandLineResolver::new();
        let mut state = test_state();
        let keymap = NotFoundKeymap;
        let input = resolve_input(&keymap);

        let result =
            resolver.resolve_with_keymap(&key_with_mod('A', Modifiers::SHIFT), &mut state, &input);
        assert!(matches!(result, ResolveResult::InsertChar { char: 'A', .. }));
    }

    #[test]
    fn test_reset_then_resolve() {
        let mut resolver = VimCommandLineResolver::new();
        resolver.reset();

        let mut state = test_state();
        let keymap = NotFoundKeymap;
        let input = resolve_input(&keymap);

        let result = resolver.resolve_with_keymap(&key('a'), &mut state, &input);
        assert!(matches!(result, ResolveResult::InsertChar { char: 'a', .. }));
    }

    #[test]
    fn test_function_keys_not_handled() {
        let resolver = VimCommandLineResolver::new();
        let mut state = test_state();
        let keymap = NotFoundKeymap;
        let input = resolve_input(&keymap);

        for code in [KeyCode::F(1), KeyCode::F(5), KeyCode::F(12)] {
            let result = resolver.resolve_with_keymap(&KeyEvent::new(code), &mut state, &input);
            assert!(
                matches!(result, ResolveResult::NotHandled),
                "Function key {code:?} should not be handled"
            );
        }
    }
}
