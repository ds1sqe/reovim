//! Vim insert mode key resolver.
//!
//! In insert mode, most keys insert characters directly. Special keys
//! like Escape exit insert mode, and control sequences trigger commands.

use {
    reovim_domain_text::Position,
    reovim_driver_text_input::{
        ExtensionMap, KeyCode, KeyEvent, KeyLookupState, KeySequence, ModeKeyResolver, ModeState,
        Modifiers, ResolveContext, ResolveInput, ResolveResult, SessionApiDyn,
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
    #[cfg_attr(coverage_nightly, coverage(off))]
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

    /// Insert mode key resolution with session access.
    ///
    /// Insertable characters are inserted directly through the session API
    /// (buffer mutation, undo recording, event emission). Non-insertable keys
    /// delegate to keymap lookup.
    ///
    /// Tracks all inserted characters in `VimSessionState.insert_buffer`
    /// for dot repeat support (Epic #465).
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn resolve_with_session(
        &self,
        key: &KeyEvent,
        state: &mut ModeState,
        input: &ResolveInput<'_>,
        session: &mut dyn SessionApiDyn,
        _shared_extensions: &mut ExtensionMap,
        client_extensions: &mut ExtensionMap,
    ) -> ResolveResult {
        // #577: Record key for dot repeat (before any early returns)
        if let Some(vim) = client_extensions.get_mut::<VimSessionState>() {
            vim.record_repeat_key(*key);
        }

        // Check for insertable character first
        if let Some(c) = Self::is_insertable(key) {
            // Track inserted character for dot repeat (Epic #465)
            if let Some(vim) = client_extensions.get_mut::<VimSessionState>() {
                vim.insert_buffer.push(c);
            }

            // Insert through session API — handles buffer mutation, undo, events
            if let (Some(buffer_id), Some(cursor)) =
                (session.active_buffer(), session.cursor_position())
            {
                let text = c.to_string();
                session.insert_text(buffer_id, cursor, &text);

                // Advance cursor after insertion
                let new_cursor = if c == '\n' {
                    Position::new(cursor.line + 1, 0)
                } else {
                    Position::new(cursor.line, cursor.column + 1)
                };
                session.set_cursor_position(new_cursor);
            }
            return ResolveResult::Completed;
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
