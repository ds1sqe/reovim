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
}
