//! Vim insert mode key resolver.
//!
//! In insert mode, most keys insert characters directly. Special keys
//! like Escape exit insert mode, and control sequences trigger commands.

use {
    reovim_driver_input::{
        ExtensionMap, KeyCode, KeyEvent, KeyLookupState, KeySequence, ModeKeyResolver, ModeState,
        Modifiers, ResolveContext, ResolveInput, ResolveResult,
    },
    reovim_kernel::api::v1::ModeId,
};

use crate::{VimSessionState, modes::VimMode};

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
            return ResolveResult::insert_char(c);
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

    /// Insert mode key resolution with extension tracking for dot repeat.
    ///
    /// This method tracks all inserted characters in `VimSessionState.insert_buffer`
    /// for dot repeat support (Epic #465).
    fn resolve_with_extensions(
        &self,
        key: &KeyEvent,
        state: &mut ModeState,
        input: &ResolveInput<'_>,
        _shared_extensions: &mut ExtensionMap,
        client_extensions: &mut ExtensionMap,
    ) -> ResolveResult {
        // Check for insertable character first
        if let Some(c) = Self::is_insertable(key) {
            // Track inserted character for dot repeat (Epic #465)
            if let Some(vim) = client_extensions.get_mut::<VimSessionState>() {
                vim.insert_buffer.push(c);
            }
            return ResolveResult::insert_char(c);
        }

        // Non-insertable key - delegate to keymap lookup
        self.resolve_with_keymap(key, state, input)
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
#[allow(clippy::doc_markdown)]
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
        assert!(matches!(result, ResolveResult::InsertChar { char: 'a', .. }));

        let result = resolver.resolve_with_keymap(&key('Z'), &mut state, &input);
        assert!(matches!(result, ResolveResult::InsertChar { char: 'Z', .. }));

        let result = resolver.resolve_with_keymap(&key('5'), &mut state, &input);
        assert!(matches!(result, ResolveResult::InsertChar { char: '5', .. }));

        let result = resolver.resolve_with_keymap(&key(' '), &mut state, &input);
        assert!(matches!(result, ResolveResult::InsertChar { char: ' ', .. }));
    }

    #[test]
    fn test_insert_tab() {
        let resolver = VimInsertResolver::new();
        let mut state = test_state();
        let keymap = NotFoundKeymap;
        let input = resolve_input(&keymap);

        let result = resolver.resolve_with_keymap(&KeyEvent::new(KeyCode::Tab), &mut state, &input);
        assert!(matches!(result, ResolveResult::InsertChar { char: '\t', .. }));
    }

    #[test]
    fn test_insert_enter() {
        let resolver = VimInsertResolver::new();
        let mut state = test_state();
        let keymap = NotFoundKeymap;
        let input = resolve_input(&keymap);

        let result =
            resolver.resolve_with_keymap(&KeyEvent::new(KeyCode::Enter), &mut state, &input);
        assert!(matches!(result, ResolveResult::InsertChar { char: '\n', .. }));
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
        assert!(matches!(result, ResolveResult::InsertChar { char: 'A', .. }));
    }

    // ========================================================================
    // Additional Insert Resolver Tests
    // ========================================================================

    #[test]
    fn test_default_impl() {
        let resolver = VimInsertResolver::default();
        assert_eq!(resolver.mode_id(), &VimMode::INSERT_ID);
    }

    #[test]
    fn test_reset_is_noop() {
        let mut resolver = VimInsertResolver::new();
        resolver.reset();
        // Should still work after reset
        assert_eq!(resolver.mode_id(), &VimMode::INSERT_ID);
    }

    #[test]
    fn test_insert_special_chars() {
        let resolver = VimInsertResolver::new();
        let mut state = test_state();
        let keymap = NotFoundKeymap;
        let input = resolve_input(&keymap);

        // Various printable characters
        let result = resolver.resolve_with_keymap(&key('!'), &mut state, &input);
        assert!(matches!(result, ResolveResult::InsertChar { char: '!', .. }));

        let result = resolver.resolve_with_keymap(&key('@'), &mut state, &input);
        assert!(matches!(result, ResolveResult::InsertChar { char: '@', .. }));

        let result = resolver.resolve_with_keymap(&key('#'), &mut state, &input);
        assert!(matches!(result, ResolveResult::InsertChar { char: '#', .. }));

        let result = resolver.resolve_with_keymap(&key('~'), &mut state, &input);
        assert!(matches!(result, ResolveResult::InsertChar { char: '~', .. }));
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_insert_digits() {
        let resolver = VimInsertResolver::new();
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
    fn test_delete_key_not_handled() {
        let resolver = VimInsertResolver::new();
        let mut state = test_state();
        let keymap = NotFoundKeymap;
        let input = resolve_input(&keymap);

        let result =
            resolver.resolve_with_keymap(&KeyEvent::new(KeyCode::Delete), &mut state, &input);
        assert!(matches!(result, ResolveResult::NotHandled));
    }

    #[test]
    fn test_home_end_not_handled() {
        let resolver = VimInsertResolver::new();
        let mut state = test_state();
        let keymap = NotFoundKeymap;
        let input = resolve_input(&keymap);

        let result =
            resolver.resolve_with_keymap(&KeyEvent::new(KeyCode::Home), &mut state, &input);
        assert!(matches!(result, ResolveResult::NotHandled));

        let result = resolver.resolve_with_keymap(&KeyEvent::new(KeyCode::End), &mut state, &input);
        assert!(matches!(result, ResolveResult::NotHandled));
    }

    #[test]
    fn test_f_keys_not_handled() {
        let resolver = VimInsertResolver::new();
        let mut state = test_state();
        let keymap = NotFoundKeymap;
        let input = resolve_input(&keymap);

        let result =
            resolver.resolve_with_keymap(&KeyEvent::new(KeyCode::F(1)), &mut state, &input);
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
        let resolver = VimInsertResolver::new();
        let mut state = test_state();
        let keymap = ExactKeymap;
        let input = resolve_input(&keymap);

        // When keymap has binding for Escape, it should execute it
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
    fn test_backspace_executes_with_keymap_binding() {
        let resolver = VimInsertResolver::new();
        let mut state = test_state();
        let keymap = ExactKeymap;
        let input = resolve_input(&keymap);

        // When keymap has binding for Backspace, it should execute it
        let result =
            resolver.resolve_with_keymap(&KeyEvent::new(KeyCode::Backspace), &mut state, &input);
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
        let resolver = VimInsertResolver::new();
        let mut state = test_state();
        let keymap = PrefixKeymap;
        let input = resolve_input(&keymap);

        let result =
            resolver.resolve_with_keymap(&KeyEvent::new(KeyCode::Escape), &mut state, &input);
        assert!(matches!(result, ResolveResult::Pending));
    }

    #[test]
    fn test_ctrl_alt_not_insertable() {
        let resolver = VimInsertResolver::new();
        let mut state = test_state();
        let keymap = NotFoundKeymap;
        let input = resolve_input(&keymap);

        // Ctrl+Alt+a should not insert
        let mods = Modifiers::CTRL | Modifiers::ALT;
        let result = resolver.resolve_with_keymap(&key_with_mod('a', mods), &mut state, &input);
        assert!(matches!(result, ResolveResult::NotHandled));
    }

    // ========================================================================
    // resolve_with_extensions tests
    // ========================================================================

    #[test]
    fn test_resolve_with_extensions_tracks_inserted_char() {
        let resolver = VimInsertResolver::new();
        let mut state = test_state();
        let keymap = NotFoundKeymap;
        let input = resolve_input(&keymap);

        let mut extensions = reovim_driver_input::ExtensionMap::new();
        let mut shared_ext = reovim_driver_input::ExtensionMap::new();
        // Pre-populate with VimSessionState
        let _ = extensions.get_or_insert::<VimSessionState>();

        let result = resolver.resolve_with_extensions(
            &key('a'),
            &mut state,
            &input,
            &mut shared_ext,
            &mut extensions,
        );
        assert!(matches!(result, ResolveResult::InsertChar { char: 'a', .. }));

        // Check insert_buffer was populated
        let vim = extensions.get::<VimSessionState>().unwrap();
        assert_eq!(vim.insert_buffer, "a");
    }

    #[test]
    fn test_resolve_with_extensions_tracks_multiple_chars() {
        let resolver = VimInsertResolver::new();
        let mut state = test_state();
        let keymap = NotFoundKeymap;
        let input = resolve_input(&keymap);

        let mut extensions = reovim_driver_input::ExtensionMap::new();
        let mut shared_ext = reovim_driver_input::ExtensionMap::new();
        let _ = extensions.get_or_insert::<VimSessionState>();

        resolver.resolve_with_extensions(
            &key('h'),
            &mut state,
            &input,
            &mut shared_ext,
            &mut extensions,
        );
        resolver.resolve_with_extensions(
            &key('e'),
            &mut state,
            &input,
            &mut shared_ext,
            &mut extensions,
        );
        resolver.resolve_with_extensions(
            &key('l'),
            &mut state,
            &input,
            &mut shared_ext,
            &mut extensions,
        );
        resolver.resolve_with_extensions(
            &key('l'),
            &mut state,
            &input,
            &mut shared_ext,
            &mut extensions,
        );
        resolver.resolve_with_extensions(
            &key('o'),
            &mut state,
            &input,
            &mut shared_ext,
            &mut extensions,
        );

        let vim = extensions.get::<VimSessionState>().unwrap();
        assert_eq!(vim.insert_buffer, "hello");
    }

    #[test]
    fn test_resolve_with_extensions_tracks_tab() {
        let resolver = VimInsertResolver::new();
        let mut state = test_state();
        let keymap = NotFoundKeymap;
        let input = resolve_input(&keymap);

        let mut extensions = reovim_driver_input::ExtensionMap::new();
        let mut shared_ext = reovim_driver_input::ExtensionMap::new();
        let _ = extensions.get_or_insert::<VimSessionState>();

        resolver.resolve_with_extensions(
            &KeyEvent::new(KeyCode::Tab),
            &mut state,
            &input,
            &mut shared_ext,
            &mut extensions,
        );

        let vim = extensions.get::<VimSessionState>().unwrap();
        assert_eq!(vim.insert_buffer, "\t");
    }

    #[test]
    fn test_resolve_with_extensions_tracks_enter() {
        let resolver = VimInsertResolver::new();
        let mut state = test_state();
        let keymap = NotFoundKeymap;
        let input = resolve_input(&keymap);

        let mut extensions = reovim_driver_input::ExtensionMap::new();
        let mut shared_ext = reovim_driver_input::ExtensionMap::new();
        let _ = extensions.get_or_insert::<VimSessionState>();

        resolver.resolve_with_extensions(
            &KeyEvent::new(KeyCode::Enter),
            &mut state,
            &input,
            &mut shared_ext,
            &mut extensions,
        );

        let vim = extensions.get::<VimSessionState>().unwrap();
        assert_eq!(vim.insert_buffer, "\n");
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_resolve_with_extensions_non_insertable_delegates_to_keymap() {
        let resolver = VimInsertResolver::new();
        let mut state = test_state();
        let keymap = ExactKeymap;
        let input = resolve_input(&keymap);

        let mut extensions = reovim_driver_input::ExtensionMap::new();
        let mut shared_ext = reovim_driver_input::ExtensionMap::new();
        let _ = extensions.get_or_insert::<VimSessionState>();

        // Escape is non-insertable - should delegate to keymap
        let result = resolver.resolve_with_extensions(
            &KeyEvent::new(KeyCode::Escape),
            &mut state,
            &input,
            &mut shared_ext,
            &mut extensions,
        );

        match result {
            ResolveResult::Execute(cmd, _) => {
                assert_eq!(cmd.name(), "test-cmd");
            }
            _ => panic!("expected Execute, got {result:?}"),
        }

        // insert_buffer should NOT have been updated
        let vim = extensions.get::<VimSessionState>().unwrap();
        assert!(vim.insert_buffer.is_empty());
    }

    #[test]
    fn test_resolve_with_extensions_ctrl_char_not_tracked() {
        let resolver = VimInsertResolver::new();
        let mut state = test_state();
        let keymap = NotFoundKeymap;
        let input = resolve_input(&keymap);

        let mut extensions = reovim_driver_input::ExtensionMap::new();
        let mut shared_ext = reovim_driver_input::ExtensionMap::new();
        let _ = extensions.get_or_insert::<VimSessionState>();

        // Ctrl+H is non-insertable - should not be tracked
        let result = resolver.resolve_with_extensions(
            &key_with_mod('h', Modifiers::CTRL),
            &mut state,
            &input,
            &mut shared_ext,
            &mut extensions,
        );
        assert!(matches!(result, ResolveResult::NotHandled));

        let vim = extensions.get::<VimSessionState>().unwrap();
        assert!(vim.insert_buffer.is_empty());
    }

    #[test]
    fn test_resolve_with_extensions_without_vim_session_state() {
        let resolver = VimInsertResolver::new();
        let mut state = test_state();
        let keymap = NotFoundKeymap;
        let input = resolve_input(&keymap);

        let mut extensions = reovim_driver_input::ExtensionMap::new();
        let mut shared_ext = reovim_driver_input::ExtensionMap::new();
        // Don't insert VimSessionState - should still work

        let result = resolver.resolve_with_extensions(
            &key('x'),
            &mut state,
            &input,
            &mut shared_ext,
            &mut extensions,
        );
        assert!(matches!(result, ResolveResult::InsertChar { char: 'x', .. }));
    }

    /// Mock keymap returning ExactWithLonger for testing.
    struct ExactWithLongerKeymap;

    #[cfg_attr(coverage_nightly, coverage(off))]
    impl KeymapQuery for ExactWithLongerKeymap {
        fn query(&self, _mode: &ModeId, _keys: &KeySequence) -> KeyLookupState {
            KeyLookupState::ExactWithLonger {
                exact: reovim_kernel::api::v1::CommandId::new(
                    reovim_kernel::api::v1::ModuleId::new("test"),
                    "longer-cmd",
                ),
            }
        }
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_exact_with_longer_executes_exact() {
        let resolver = VimInsertResolver::new();
        let mut state = test_state();
        let keymap = ExactWithLongerKeymap;
        let input = resolve_input(&keymap);

        // Non-insertable key with ExactWithLonger should execute exact match
        let result =
            resolver.resolve_with_keymap(&KeyEvent::new(KeyCode::Escape), &mut state, &input);
        match result {
            ResolveResult::Execute(cmd, _) => {
                assert_eq!(cmd.name(), "longer-cmd");
            }
            _ => panic!("expected Execute, got {result:?}"),
        }
    }
}
