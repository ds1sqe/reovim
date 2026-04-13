//! Replace mode key resolver.
//!
//! In replace mode (`R`), typed characters overwrite existing text at the
//! cursor position. Backspace restores the original character that was
//! overwritten. At end-of-line, characters are inserted (extending the line).
//!
//! The restore stack is stored in `VimSessionState` so that both the resolver
//! and the `ReplaceBackspace` command handler can access it.

use {
    reovim_domain_text::Position,
    reovim_driver_input::{
        ExtensionMap, KeyCode, KeyEvent, KeyLookupState, KeySequence, ModeKeyResolver, ModeState,
        Modifiers, ResolveContext, ResolveInput, ResolveResult, SessionApiDyn,
    },
    reovim_kernel::api::v1::ModeId,
};

use crate::{VimSessionState, ids, modes::VimMode, session_state::ReplaceRestoreEntry};

/// Vim replace mode key resolver.
///
/// Handles `R` mode: typed characters overwrite existing text, backspace
/// restores originals. Inherits from insert mode for Escape, arrow keys,
/// and other non-insertable keys.
pub struct VimReplaceResolver {
    /// Mode ID for replace mode.
    mode_id: ModeId,
    /// Parent mode ID (insert mode) for inheritance.
    parent_mode_id: ModeId,
}

impl VimReplaceResolver {
    /// Create a new replace mode resolver.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            mode_id: VimMode::REPLACE_ID,
            parent_mode_id: VimMode::INSERT_ID,
        }
    }

    /// Check if a key should insert a character (same logic as insert mode).
    const fn is_insertable(key: &KeyEvent) -> Option<char> {
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

    /// Check if a key is backspace.
    const fn is_backspace(key: &KeyEvent) -> bool {
        matches!(key.code, KeyCode::Backspace)
            || (key.modifiers.contains(Modifiers::CTRL) && matches!(key.code, KeyCode::Char('h')))
    }
}

impl Default for VimReplaceResolver {
    fn default() -> Self {
        Self::new()
    }
}

impl ModeKeyResolver for VimReplaceResolver {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn resolve_with_session(
        &self,
        key: &KeyEvent,
        _state: &mut ModeState,
        input: &ResolveInput<'_>,
        session: &mut dyn SessionApiDyn,
        _shared_extensions: &mut ExtensionMap,
        client_extensions: &mut ExtensionMap,
    ) -> ResolveResult {
        // Record key for dot repeat
        if let Some(vim) = client_extensions.get_mut::<VimSessionState>() {
            vim.record_repeat_key(*key);
        }

        // Handle backspace: dispatch ReplaceBackspace command
        if Self::is_backspace(key) {
            return ResolveResult::Execute(ids::REPLACE_BACKSPACE, ResolveContext::new());
        }

        // Handle insertable characters: overwrite at cursor
        if let Some(c) = Self::is_insertable(key) {
            // Track inserted character for dot repeat
            if let Some(vim) = client_extensions.get_mut::<VimSessionState>() {
                vim.insert_buffer.push(c);
            }

            // For newline, just insert (don't overwrite) — vim behavior
            if c == '\n' {
                push_restore_entry(session, client_extensions);
                if let (Some(buffer_id), Some(cursor)) =
                    (session.active_buffer(), session.cursor_position())
                {
                    session.insert_text(buffer_id, cursor, "\n");
                    session.set_cursor_position(Position::new(cursor.line + 1, 0));
                }
                return ResolveResult::Completed;
            }

            return handle_replace_char(session, client_extensions, c);
        }

        // Non-insertable key: delegate to keymap (Escape, arrows, etc.)
        let keys = KeySequence::from_keys(&[*key]);
        let lookup_state = input.keymap.query(input.mode, &keys);

        match lookup_state {
            KeyLookupState::ExactOnly(cmd) | KeyLookupState::ExactWithLonger { exact: cmd, .. } => {
                ResolveResult::Execute(cmd, ResolveContext::new())
            }
            KeyLookupState::PrefixOnly => ResolveResult::Pending,
            KeyLookupState::NotFound => ResolveResult::NotHandled,
        }
    }

    fn mode_id(&self) -> &ModeId {
        &self.mode_id
    }

    fn inherits_from(&self) -> Option<&ModeId> {
        Some(&self.parent_mode_id)
    }

    fn reset(&mut self) {
        // Restore stack is in VimSessionState, cleared on mode entry
    }
}

/// Push a restore entry for the current cursor position.
#[cfg_attr(coverage_nightly, coverage(off))]
fn push_restore_entry(session: &dyn SessionApiDyn, client_extensions: &mut ExtensionMap) {
    let Some(buffer_id) = session.active_buffer() else {
        return;
    };
    let Some(cursor) = session.cursor_position() else {
        return;
    };

    let line_len = session.buffer_line_len(buffer_id, cursor.line).unwrap_or(0);
    let original = if cursor.column < line_len {
        session
            .buffer_line(buffer_id, cursor.line)
            .and_then(|line| line.chars().nth(cursor.column))
    } else {
        None
    };

    if let Some(vim) = client_extensions.get_mut::<VimSessionState>() {
        vim.replace_restore_stack.push(ReplaceRestoreEntry {
            position: cursor,
            original,
        });
    }
}

/// Handle a regular character replacement at the cursor position.
#[cfg_attr(coverage_nightly, coverage(off))]
fn handle_replace_char(
    session: &mut dyn SessionApiDyn,
    client_extensions: &mut ExtensionMap,
    replacement: char,
) -> ResolveResult {
    let Some(buffer_id) = session.active_buffer() else {
        return ResolveResult::Completed;
    };

    let Some(cursor) = session.cursor_position() else {
        return ResolveResult::Completed;
    };

    let line_len = session.buffer_line_len(buffer_id, cursor.line).unwrap_or(0);

    if cursor.column < line_len {
        // Within line: read original char, delete it, then insert replacement
        let original_char = session
            .buffer_line(buffer_id, cursor.line)
            .and_then(|line| line.chars().nth(cursor.column));

        if let Some(vim) = client_extensions.get_mut::<VimSessionState>() {
            vim.replace_restore_stack.push(ReplaceRestoreEntry {
                position: cursor,
                original: original_char,
            });
        }

        // Delete the character at cursor
        let char_end = cursor.column + original_char.map_or(1, char::len_utf8);
        session.delete_range(buffer_id, cursor, Position::new(cursor.line, char_end));
    } else {
        // At end of line: just insert (extends line)
        if let Some(vim) = client_extensions.get_mut::<VimSessionState>() {
            vim.replace_restore_stack.push(ReplaceRestoreEntry {
                position: cursor,
                original: None,
            });
        }
    }

    // Insert the replacement character through session API
    let text = replacement.to_string();
    session.insert_text(buffer_id, cursor, &text);
    session.set_cursor_position(Position::new(cursor.line, cursor.column + 1));
    ResolveResult::Completed
}
