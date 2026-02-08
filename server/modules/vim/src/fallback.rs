//! Vim fallback handler for character insertion.
//!
//! This handler provides the policy for unmatched keys:
//! - In Insert mode: Insert the character into the buffer
//! - In Normal mode: Beep (invalid key)
//!
//! # Epic #372 - Mode Ownership
//!
//! This handler uses `VimMode::*_ID` constants directly to check the current
//! mode, which is why it belongs in the vim module rather than the generic
//! editor module.
//!
//! # Design Philosophy
//!
//! The fallback handler is a **policy** component. The event loop (mechanism)
//! doesn't know about Insert mode or character insertion - it just delegates
//! to this handler when a key doesn't match any binding.
//!
//! Tab, Enter, Backspace, and Delete are NOT handled here because they have
//! explicit keybindings to commands in insert mode (see keymap/insert.rs).

use reovim_driver_input::{
    FallbackContext, FallbackResult, InputFallbackHandler, KeyCode, KeyEvent, Modifiers,
};

use crate::modes::VimMode;

/// Vim-specific fallback handler.
///
/// Implements character insertion for Insert mode and beeps for
/// unmatched keys in Normal mode.
///
/// # Design Philosophy
///
/// This is a **policy** implementation. The event loop (mechanism) doesn't
/// know about Insert mode or character insertion - it just delegates to
/// this handler when a key doesn't match any binding.
///
/// # Example
///
/// ```ignore
/// use reovim_module_vim::VimFallbackHandler;
/// use runner::EventLoop;
///
/// let fallback = VimFallbackHandler;
/// let event_loop = EventLoop::new(app, modes, commands, keymaps, fallback);
/// ```
#[derive(Debug, Clone, Copy, Default)]
pub struct VimFallbackHandler;

impl<C: FallbackContext> InputFallbackHandler<C> for VimFallbackHandler {
    fn handle_unmatched(&self, key: KeyEvent, ctx: &mut C) -> FallbackResult {
        let mode_id = ctx.current_mode();

        // Check if we're in Insert mode
        if *mode_id == VimMode::INSERT_ID {
            // Try to extract a printable character
            if let Some(ch) = key_to_char(&key) {
                // Insert the character into the active buffer
                if let Some(buffer_id) = ctx.active_buffer()
                    && let Some(cursor_before) = ctx.cursor_position()
                    && let Some(buffer_arc) = ctx.get_buffer(buffer_id)
                {
                    let mut buffer = buffer_arc.write();
                    // Insert character at cursor position
                    let text = ch.to_string();
                    buffer.insert_at(cursor_before, &text);
                    drop(buffer);

                    // Calculate cursor after insert (advance by text length)
                    let cursor_after = if ch == '\n' {
                        reovim_kernel::api::v1::Position::new(cursor_before.line + 1, 0)
                    } else {
                        reovim_kernel::api::v1::Position::new(
                            cursor_before.line,
                            cursor_before.column + 1,
                        )
                    };

                    // Update cursor position
                    ctx.set_cursor_position(cursor_after);

                    // Accumulate edit for batched undo tracking
                    // (consecutive inserts become single undo node)
                    let edit = reovim_kernel::api::v1::Edit::insert(cursor_before, &text);
                    ctx.accumulate_edit(buffer_id, edit, cursor_before, cursor_after);

                    return FallbackResult::Handled;
                }
                return FallbackResult::Handled;
            }

            // Non-printable key in Insert mode - ignore it
            return FallbackResult::Ignored;
        }

        // In Normal mode, unmatched keys should beep
        if *mode_id == VimMode::NORMAL_ID {
            return FallbackResult::Beep;
        }

        // Unknown mode - ignore
        FallbackResult::Ignored
    }
}

/// Extract a printable character from a key event.
///
/// Returns `Some(char)` if the key is a printable character without
/// modifiers (or with just Shift for uppercase).
fn key_to_char(key: &KeyEvent) -> Option<char> {
    // Only handle key press events
    if !key.is_press() {
        return None;
    }

    // Check for printable character
    match key.code {
        KeyCode::Char(ch) => {
            // Allow character with no modifiers or just shift
            if key.modifiers.is_empty() || key.modifiers == Modifiers::SHIFT {
                Some(ch)
            } else {
                None
            }
        }
        KeyCode::Tab => Some('\t'),
        KeyCode::Enter => Some('\n'),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use std::{collections::HashMap, sync::Arc};

    use reovim_kernel::api::v1::{Buffer, BufferId, Edit, ModeId, Position, RwLock};

    use super::*;

    /// Test context that implements `FallbackContext`.
    struct TestContext {
        mode: ModeId,
        active_buffer: Option<BufferId>,
        buffers: HashMap<BufferId, Arc<RwLock<Buffer>>>,
        recorded_edits: Vec<(BufferId, Vec<Edit>, Position, Position)>,
    }

    impl TestContext {
        fn with_mode(mode: ModeId) -> Self {
            Self {
                mode,
                active_buffer: None,
                buffers: HashMap::new(),
                recorded_edits: Vec::new(),
            }
        }

        fn normal() -> Self {
            Self::with_mode(VimMode::NORMAL_ID)
        }

        fn insert() -> Self {
            Self::with_mode(VimMode::INSERT_ID)
        }
    }

    impl FallbackContext for TestContext {
        fn current_mode(&self) -> &ModeId {
            &self.mode
        }

        fn active_buffer(&self) -> Option<BufferId> {
            self.active_buffer
        }

        fn cursor_position(&self) -> Option<Position> {
            Some(Position::origin())
        }

        fn set_cursor_position(&mut self, _pos: Position) {
            // No-op in mock
        }

        fn get_buffer(&self, id: BufferId) -> Option<Arc<RwLock<Buffer>>> {
            self.buffers.get(&id).cloned()
        }

        fn record_edit(
            &mut self,
            buffer_id: BufferId,
            edits: Vec<Edit>,
            cursor_before: Position,
            cursor_after: Position,
        ) {
            self.recorded_edits
                .push((buffer_id, edits, cursor_before, cursor_after));
        }

        fn accumulate_edit(
            &mut self,
            buffer_id: BufferId,
            edit: Edit,
            cursor_before: Position,
            cursor_after: Position,
        ) {
            // For testing, just record immediately (no actual batching in mock)
            self.recorded_edits
                .push((buffer_id, vec![edit], cursor_before, cursor_after));
        }

        fn flush_pending_edits(&mut self) {
            // No-op in mock - edits recorded immediately in accumulate_edit
        }
    }

    #[test]
    fn test_normal_mode_beeps() {
        let handler = VimFallbackHandler;
        let mut ctx = TestContext::normal();

        let key = KeyEvent::new(KeyCode::Char('x'));
        let result = handler.handle_unmatched(key, &mut ctx);

        assert_eq!(result, FallbackResult::Beep);
    }

    #[test]
    fn test_insert_mode_handles_char() {
        let handler = VimFallbackHandler;
        let mut ctx = TestContext::insert();

        let key = KeyEvent::new(KeyCode::Char('a'));
        let result = handler.handle_unmatched(key, &mut ctx);

        assert_eq!(result, FallbackResult::Handled);
    }

    #[test]
    fn test_insert_mode_handles_uppercase() {
        let handler = VimFallbackHandler;
        let mut ctx = TestContext::insert();

        let key = KeyEvent::with_modifiers(KeyCode::Char('A'), Modifiers::SHIFT);
        let result = handler.handle_unmatched(key, &mut ctx);

        assert_eq!(result, FallbackResult::Handled);
    }

    #[test]
    fn test_insert_mode_ignores_ctrl_char() {
        let handler = VimFallbackHandler;
        let mut ctx = TestContext::insert();

        let key = KeyEvent::with_modifiers(KeyCode::Char('c'), Modifiers::CTRL);
        let result = handler.handle_unmatched(key, &mut ctx);

        assert_eq!(result, FallbackResult::Ignored);
    }

    #[test]
    fn test_insert_mode_handles_tab() {
        let handler = VimFallbackHandler;
        let mut ctx = TestContext::insert();

        let key = KeyEvent::new(KeyCode::Tab);
        let result = handler.handle_unmatched(key, &mut ctx);

        assert_eq!(result, FallbackResult::Handled);
    }

    #[test]
    fn test_insert_mode_handles_enter() {
        let handler = VimFallbackHandler;
        let mut ctx = TestContext::insert();

        let key = KeyEvent::new(KeyCode::Enter);
        let result = handler.handle_unmatched(key, &mut ctx);

        assert_eq!(result, FallbackResult::Handled);
    }

    #[test]
    fn test_insert_mode_ignores_special_keys() {
        let handler = VimFallbackHandler;
        let mut ctx = TestContext::insert();

        // Function keys, arrow keys, etc. should be ignored
        let key = KeyEvent::new(KeyCode::F(1));
        let result = handler.handle_unmatched(key, &mut ctx);

        assert_eq!(result, FallbackResult::Ignored);
    }

    #[test]
    fn test_key_to_char_basic() {
        let key = KeyEvent::new(KeyCode::Char('a'));
        assert_eq!(key_to_char(&key), Some('a'));
    }

    #[test]
    fn test_key_to_char_shift() {
        let key = KeyEvent::with_modifiers(KeyCode::Char('A'), Modifiers::SHIFT);
        assert_eq!(key_to_char(&key), Some('A'));
    }

    #[test]
    fn test_key_to_char_ctrl_rejected() {
        let key = KeyEvent::with_modifiers(KeyCode::Char('c'), Modifiers::CTRL);
        assert_eq!(key_to_char(&key), None);
    }

    #[test]
    fn test_key_to_char_release_rejected() {
        use reovim_driver_input::KeyEventKind;
        let key = KeyEvent::full(KeyCode::Char('a'), Modifiers::NONE, KeyEventKind::Release);
        assert_eq!(key_to_char(&key), None);
    }

    #[test]
    fn test_key_to_char_tab() {
        let key = KeyEvent::new(KeyCode::Tab);
        assert_eq!(key_to_char(&key), Some('\t'));
    }

    #[test]
    fn test_key_to_char_enter() {
        let key = KeyEvent::new(KeyCode::Enter);
        assert_eq!(key_to_char(&key), Some('\n'));
    }

    #[test]
    fn test_unknown_mode_ignored() {
        let handler = VimFallbackHandler;
        let mode = ModeId::new(reovim_kernel::api::v1::ModuleId::new("other"), "unknown");
        let mut ctx = TestContext::with_mode(mode);

        let key = KeyEvent::new(KeyCode::Char('a'));
        let result = handler.handle_unmatched(key, &mut ctx);

        assert_eq!(result, FallbackResult::Ignored);
    }

    #[test]
    fn test_insert_mode_with_buffer_inserts_char() {
        let handler = VimFallbackHandler;
        let mut ctx = TestContext::insert();

        // Set up an actual buffer
        let buffer_id = BufferId::from_raw(1);
        let buffer = Buffer::from_string("hello");
        ctx.active_buffer = Some(buffer_id);
        ctx.buffers.insert(buffer_id, Arc::new(RwLock::new(buffer)));

        let key = KeyEvent::new(KeyCode::Char('x'));
        let result = handler.handle_unmatched(key, &mut ctx);

        assert_eq!(result, FallbackResult::Handled);

        // Check the buffer was modified
        let buf = ctx.buffers.get(&buffer_id).unwrap();
        let content = buf.read().lines()[0].clone();
        assert_eq!(content, "xhello");
    }

    #[test]
    fn test_key_to_char_alt_rejected() {
        let key = KeyEvent::with_modifiers(KeyCode::Char('a'), Modifiers::ALT);
        assert_eq!(key_to_char(&key), None);
    }

    #[test]
    fn test_key_to_char_ctrl_shift_rejected() {
        let key = KeyEvent::with_modifiers(KeyCode::Char('a'), Modifiers::CTRL | Modifiers::SHIFT);
        assert_eq!(key_to_char(&key), None);
    }

    #[test]
    fn test_key_to_char_arrow_key() {
        let key = KeyEvent::new(KeyCode::Left);
        assert_eq!(key_to_char(&key), None);
    }

    #[test]
    fn test_key_to_char_home_key() {
        let key = KeyEvent::new(KeyCode::Home);
        assert_eq!(key_to_char(&key), None);
    }

    #[test]
    fn test_key_to_char_backspace() {
        let key = KeyEvent::new(KeyCode::Backspace);
        assert_eq!(key_to_char(&key), None);
    }

    #[test]
    fn test_key_to_char_delete() {
        let key = KeyEvent::new(KeyCode::Delete);
        assert_eq!(key_to_char(&key), None);
    }

    #[test]
    fn test_normal_mode_beeps_for_special_key() {
        let handler = VimFallbackHandler;
        let mut ctx = TestContext::normal();

        let key = KeyEvent::new(KeyCode::Home);
        let result = handler.handle_unmatched(key, &mut ctx);

        assert_eq!(result, FallbackResult::Beep);
    }

    #[test]
    fn test_insert_mode_ignores_escape() {
        let handler = VimFallbackHandler;
        let mut ctx = TestContext::insert();

        let key = KeyEvent::new(KeyCode::Escape);
        let result = handler.handle_unmatched(key, &mut ctx);

        assert_eq!(result, FallbackResult::Ignored);
    }

    #[test]
    fn test_fallback_handler_debug() {
        let handler = VimFallbackHandler;
        assert!(format!("{handler:?}").contains("VimFallbackHandler"));
    }

    #[test]
    fn test_fallback_handler_default() {
        let _ = VimFallbackHandler;
    }

    #[test]
    fn test_fallback_handler_clone() {
        let handler = VimFallbackHandler;
        let _ = handler;
    }

    #[test]
    fn test_insert_mode_handles_digit() {
        let handler = VimFallbackHandler;
        let mut ctx = TestContext::insert();

        let key = KeyEvent::new(KeyCode::Char('5'));
        let result = handler.handle_unmatched(key, &mut ctx);

        assert_eq!(result, FallbackResult::Handled);
    }

    #[test]
    fn test_insert_mode_handles_space() {
        let handler = VimFallbackHandler;
        let mut ctx = TestContext::insert();

        let key = KeyEvent::new(KeyCode::Char(' '));
        let result = handler.handle_unmatched(key, &mut ctx);

        assert_eq!(result, FallbackResult::Handled);
    }

    #[test]
    fn test_insert_mode_handles_symbol() {
        let handler = VimFallbackHandler;
        let mut ctx = TestContext::insert();

        let key = KeyEvent::new(KeyCode::Char('!'));
        let result = handler.handle_unmatched(key, &mut ctx);

        assert_eq!(result, FallbackResult::Handled);
    }

    #[test]
    fn test_insert_mode_handles_unicode() {
        let handler = VimFallbackHandler;
        let mut ctx = TestContext::insert();

        // Unicode characters should be handled
        let key = KeyEvent::new(KeyCode::Char('\u{00e9}')); // e with accent
        let result = handler.handle_unmatched(key, &mut ctx);

        assert_eq!(result, FallbackResult::Handled);
    }

    // ========================================================================
    // Additional edge case tests
    // ========================================================================

    #[test]
    fn test_key_to_char_end_key() {
        let key = KeyEvent::new(KeyCode::End);
        assert_eq!(key_to_char(&key), None);
    }

    #[test]
    fn test_key_to_char_page_up() {
        let key = KeyEvent::new(KeyCode::PageUp);
        assert_eq!(key_to_char(&key), None);
    }

    #[test]
    fn test_key_to_char_page_down() {
        let key = KeyEvent::new(KeyCode::PageDown);
        assert_eq!(key_to_char(&key), None);
    }

    #[test]
    fn test_key_to_char_insert_key() {
        let key = KeyEvent::new(KeyCode::Insert);
        assert_eq!(key_to_char(&key), None);
    }

    #[test]
    fn test_key_to_char_f12() {
        let key = KeyEvent::new(KeyCode::F(12));
        assert_eq!(key_to_char(&key), None);
    }

    #[test]
    fn test_key_to_char_up_arrow() {
        let key = KeyEvent::new(KeyCode::Up);
        assert_eq!(key_to_char(&key), None);
    }

    #[test]
    fn test_key_to_char_down_arrow() {
        let key = KeyEvent::new(KeyCode::Down);
        assert_eq!(key_to_char(&key), None);
    }

    #[test]
    fn test_key_to_char_right_arrow() {
        let key = KeyEvent::new(KeyCode::Right);
        assert_eq!(key_to_char(&key), None);
    }

    #[test]
    fn test_normal_mode_beeps_for_digit() {
        let handler = VimFallbackHandler;
        let mut ctx = TestContext::normal();

        let key = KeyEvent::new(KeyCode::Char('5'));
        let result = handler.handle_unmatched(key, &mut ctx);

        assert_eq!(result, FallbackResult::Beep);
    }

    #[test]
    fn test_normal_mode_beeps_for_enter() {
        let handler = VimFallbackHandler;
        let mut ctx = TestContext::normal();

        let key = KeyEvent::new(KeyCode::Enter);
        let result = handler.handle_unmatched(key, &mut ctx);

        assert_eq!(result, FallbackResult::Beep);
    }

    #[test]
    fn test_insert_mode_with_buffer_inserts_at_origin() {
        let handler = VimFallbackHandler;
        let mut ctx = TestContext::insert();

        let buffer_id = BufferId::from_raw(2);
        let buffer = Buffer::from_string("abc");
        ctx.active_buffer = Some(buffer_id);
        ctx.buffers.insert(buffer_id, Arc::new(RwLock::new(buffer)));

        let key = KeyEvent::new(KeyCode::Char('Z'));
        let result = handler.handle_unmatched(key, &mut ctx);
        assert_eq!(result, FallbackResult::Handled);

        // Check edit was recorded
        assert!(!ctx.recorded_edits.is_empty());
    }

    #[test]
    fn test_insert_mode_with_buffer_records_edit() {
        let handler = VimFallbackHandler;
        let mut ctx = TestContext::insert();

        let buffer_id = BufferId::from_raw(3);
        let buffer = Buffer::from_string("test");
        ctx.active_buffer = Some(buffer_id);
        ctx.buffers.insert(buffer_id, Arc::new(RwLock::new(buffer)));

        let key = KeyEvent::new(KeyCode::Char('X'));
        handler.handle_unmatched(key, &mut ctx);

        // Verify an edit was recorded
        assert_eq!(ctx.recorded_edits.len(), 1);
        let (recorded_buffer_id, _edits, cursor_before, _cursor_after) = &ctx.recorded_edits[0];
        assert_eq!(*recorded_buffer_id, buffer_id);
        assert_eq!(*cursor_before, Position::origin());
    }

    #[test]
    fn test_insert_mode_ignores_up_arrow() {
        let handler = VimFallbackHandler;
        let mut ctx = TestContext::insert();

        let key = KeyEvent::new(KeyCode::Up);
        let result = handler.handle_unmatched(key, &mut ctx);
        assert_eq!(result, FallbackResult::Ignored);
    }

    #[test]
    fn test_insert_mode_ignores_down_arrow() {
        let handler = VimFallbackHandler;
        let mut ctx = TestContext::insert();

        let key = KeyEvent::new(KeyCode::Down);
        let result = handler.handle_unmatched(key, &mut ctx);
        assert_eq!(result, FallbackResult::Ignored);
    }

    #[test]
    fn test_key_to_char_alt_shift_rejected() {
        let key = KeyEvent::with_modifiers(KeyCode::Char('a'), Modifiers::ALT | Modifiers::SHIFT);
        assert_eq!(key_to_char(&key), None);
    }

    #[test]
    fn test_key_to_char_number_keys() {
        for digit in '0'..='9' {
            let key = KeyEvent::new(KeyCode::Char(digit));
            assert_eq!(key_to_char(&key), Some(digit));
        }
    }

    #[test]
    fn test_key_to_char_special_chars() {
        for ch in ['!', '@', '#', '$', '%', '^', '&', '*', '(', ')'] {
            let key = KeyEvent::new(KeyCode::Char(ch));
            assert_eq!(key_to_char(&key), Some(ch), "Failed for char: {ch}");
        }
    }

    // ========================================================================
    // Newline insertion test (exercises cursor_after newline branch)
    // ========================================================================

    #[test]
    fn test_insert_mode_newline_with_buffer() {
        let handler = VimFallbackHandler;
        let mut ctx = TestContext::insert();

        let buffer_id = BufferId::from_raw(10);
        let buffer = Buffer::from_string("hello");
        ctx.active_buffer = Some(buffer_id);
        ctx.buffers.insert(buffer_id, Arc::new(RwLock::new(buffer)));

        // Insert newline character (Enter key)
        let key = KeyEvent::new(KeyCode::Enter);
        let result = handler.handle_unmatched(key, &mut ctx);
        assert_eq!(result, FallbackResult::Handled);

        // Buffer should have newline inserted
        let buf = ctx.buffers.get(&buffer_id).unwrap();
        let lines = buf.read().lines().to_vec();
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0], "");
        assert_eq!(lines[1], "hello");
    }

    #[test]
    fn test_insert_mode_tab_with_buffer() {
        let handler = VimFallbackHandler;
        let mut ctx = TestContext::insert();

        let buffer_id = BufferId::from_raw(11);
        let buffer = Buffer::from_string("hello");
        ctx.active_buffer = Some(buffer_id);
        ctx.buffers.insert(buffer_id, Arc::new(RwLock::new(buffer)));

        // Insert tab character
        let key = KeyEvent::new(KeyCode::Tab);
        let result = handler.handle_unmatched(key, &mut ctx);
        assert_eq!(result, FallbackResult::Handled);

        let buf = ctx.buffers.get(&buffer_id).unwrap();
        let content = buf.read().lines()[0].clone();
        assert_eq!(content, "\thello");
    }

    #[test]
    fn test_insert_mode_newline_records_edit() {
        let handler = VimFallbackHandler;
        let mut ctx = TestContext::insert();

        let buffer_id = BufferId::from_raw(12);
        let buffer = Buffer::from_string("test");
        ctx.active_buffer = Some(buffer_id);
        ctx.buffers.insert(buffer_id, Arc::new(RwLock::new(buffer)));

        let key = KeyEvent::new(KeyCode::Enter);
        handler.handle_unmatched(key, &mut ctx);

        // Edit should be recorded with correct cursor_after for newline
        assert_eq!(ctx.recorded_edits.len(), 1);
        let (_bid, _edits, cursor_before, cursor_after) = &ctx.recorded_edits[0];
        assert_eq!(*cursor_before, Position::origin());
        // After inserting newline at (0,0), cursor should be at (1,0)
        assert_eq!(*cursor_after, Position::new(1, 0));
    }

    #[test]
    fn test_insert_mode_char_records_edit_cursor_after() {
        let handler = VimFallbackHandler;
        let mut ctx = TestContext::insert();

        let buffer_id = BufferId::from_raw(13);
        let buffer = Buffer::from_string("test");
        ctx.active_buffer = Some(buffer_id);
        ctx.buffers.insert(buffer_id, Arc::new(RwLock::new(buffer)));

        let key = KeyEvent::new(KeyCode::Char('x'));
        handler.handle_unmatched(key, &mut ctx);

        assert_eq!(ctx.recorded_edits.len(), 1);
        let (_bid, _edits, cursor_before, cursor_after) = &ctx.recorded_edits[0];
        assert_eq!(*cursor_before, Position::origin());
        // After inserting 'x' at (0,0), cursor should be at (0,1)
        assert_eq!(*cursor_after, Position::new(0, 1));
    }

    #[test]
    fn test_insert_mode_buffer_id_set_but_no_buffer() {
        let handler = VimFallbackHandler;
        let mut ctx = TestContext::insert();

        // Set active_buffer but don't add it to the map
        ctx.active_buffer = Some(BufferId::from_raw(999));

        let key = KeyEvent::new(KeyCode::Char('a'));
        let result = handler.handle_unmatched(key, &mut ctx);
        // Should return Handled even though buffer not found (early return)
        assert_eq!(result, FallbackResult::Handled);
    }
}
