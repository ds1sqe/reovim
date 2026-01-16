//! Editor fallback handler for character insertion.
//!
//! This handler provides the policy for unmatched keys:
//! - In Insert mode: Insert the character into the buffer
//! - In Normal mode: Beep (invalid key)
//!
//! # Design Philosophy
//!
//! The fallback handler is a **policy** component. The event loop (mechanism)
//! doesn't know about Insert mode or character insertion - it just delegates
//! to this handler when a key doesn't match any binding.
//!
//! Tab, Enter, Backspace, and Delete are NOT handled here because they have
//! explicit keybindings to commands in insert mode (see keymap/insert.rs).

use {
    reovim_driver_command::CommandResult,
    reovim_driver_input::{KeyCode, KeyEvent},
    runner::{AppState, FallbackResult, InputFallbackHandler},
};

use super::mode::EditorMode;

/// Editor-specific fallback handler.
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
/// use reovim_module_editor::EditorFallbackHandler;
/// use runner::EventLoop;
///
/// let fallback = EditorFallbackHandler;
/// let event_loop = EventLoop::new(app, modes, commands, keymaps, fallback);
/// ```
#[derive(Debug, Clone, Copy, Default)]
pub struct EditorFallbackHandler;

impl InputFallbackHandler for EditorFallbackHandler {
    fn handle_unmatched(
        &self,
        key: KeyEvent,
        app: &mut AppState,
    ) -> (FallbackResult, Option<CommandResult>) {
        let mode_id = app.current_mode();

        // Check if we're in Insert mode
        if *mode_id == EditorMode::INSERT_ID {
            // Try to extract a printable character
            if let Some(ch) = key_to_char(&key) {
                // Insert the character into the active buffer
                if let Some(buffer_id) = app.active_buffer
                    && let Some(buffer_arc) = app.kernel.buffers.get(buffer_id)
                {
                    let mut buffer = buffer_arc.write();
                    let cursor_before = buffer.position();
                    // Insert character at cursor position
                    let edit = buffer.insert(&ch.to_string());
                    let cursor_after = buffer.position();
                    drop(buffer);

                    // Return EditAction for undo tracking
                    return (
                        FallbackResult::Handled,
                        Some(CommandResult::edit_action(
                            buffer_id,
                            edit,
                            cursor_before,
                            cursor_after,
                        )),
                    );
                }
                return (FallbackResult::Handled, None);
            }

            // Non-printable key in Insert mode - ignore it
            return (FallbackResult::Ignored, None);
        }

        // In Normal mode, unmatched keys should beep
        if *mode_id == EditorMode::NORMAL_ID {
            return (FallbackResult::Beep, None);
        }

        // Unknown mode - ignore
        (FallbackResult::Ignored, None)
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
            if key.modifiers.is_empty() || key.modifiers == reovim_driver_input::Modifiers::SHIFT {
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
    use {
        super::*,
        reovim_driver_input::{KeyCode, KeyEvent, KeyEventKind, Modifiers},
        reovim_kernel::api::v1::KernelContext,
    };

    fn create_app_in_mode(mode_id: reovim_kernel::api::v1::ModeId) -> AppState {
        let kernel = KernelContext::default();
        AppState::new(kernel, mode_id)
    }

    fn create_app_normal() -> AppState {
        create_app_in_mode(EditorMode::NORMAL_ID)
    }

    fn create_app_insert() -> AppState {
        create_app_in_mode(EditorMode::INSERT_ID)
    }

    #[test]
    fn test_normal_mode_beeps() {
        let handler = EditorFallbackHandler;
        let mut app = create_app_normal();

        let key = KeyEvent::new(KeyCode::Char('x'));
        let (result, cmd_result) = handler.handle_unmatched(key, &mut app);

        assert_eq!(result, FallbackResult::Beep);
        assert!(cmd_result.is_none());
    }

    #[test]
    fn test_insert_mode_handles_char() {
        let handler = EditorFallbackHandler;
        let mut app = create_app_insert();

        let key = KeyEvent::new(KeyCode::Char('a'));
        let (result, _cmd_result) = handler.handle_unmatched(key, &mut app);

        assert_eq!(result, FallbackResult::Handled);
        // cmd_result is None because no buffer is set up
    }

    #[test]
    fn test_insert_mode_handles_uppercase() {
        let handler = EditorFallbackHandler;
        let mut app = create_app_insert();

        let key = KeyEvent::with_modifiers(KeyCode::Char('A'), Modifiers::SHIFT);
        let (result, _cmd_result) = handler.handle_unmatched(key, &mut app);

        assert_eq!(result, FallbackResult::Handled);
    }

    #[test]
    fn test_insert_mode_ignores_ctrl_char() {
        let handler = EditorFallbackHandler;
        let mut app = create_app_insert();

        let key = KeyEvent::with_modifiers(KeyCode::Char('c'), Modifiers::CTRL);
        let (result, cmd_result) = handler.handle_unmatched(key, &mut app);

        assert_eq!(result, FallbackResult::Ignored);
        assert!(cmd_result.is_none());
    }

    #[test]
    fn test_insert_mode_handles_tab() {
        let handler = EditorFallbackHandler;
        let mut app = create_app_insert();

        let key = KeyEvent::new(KeyCode::Tab);
        let (result, _cmd_result) = handler.handle_unmatched(key, &mut app);

        assert_eq!(result, FallbackResult::Handled);
    }

    #[test]
    fn test_insert_mode_handles_enter() {
        let handler = EditorFallbackHandler;
        let mut app = create_app_insert();

        let key = KeyEvent::new(KeyCode::Enter);
        let (result, _cmd_result) = handler.handle_unmatched(key, &mut app);

        assert_eq!(result, FallbackResult::Handled);
    }

    #[test]
    fn test_insert_mode_ignores_special_keys() {
        let handler = EditorFallbackHandler;
        let mut app = create_app_insert();

        // Function keys, arrow keys, etc. should be ignored
        let key = KeyEvent::new(KeyCode::F(1));
        let (result, cmd_result) = handler.handle_unmatched(key, &mut app);

        assert_eq!(result, FallbackResult::Ignored);
        assert!(cmd_result.is_none());
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
