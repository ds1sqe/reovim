//! Editor fallback handler for character insertion.
//!
//! This handler provides the policy for unmatched keys:
//! - In Insert mode: Insert the character into the buffer
//! - In Normal mode: Beep (invalid key)

use {
    reovim_driver_input::{KeyCode, KeyEvent},
    runner_new::{AppState, FallbackResult, InputFallbackHandler},
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
/// use runner_new::EventLoop;
///
/// let fallback = EditorFallbackHandler;
/// let event_loop = EventLoop::new(app, modes, commands, keymaps, fallback);
/// ```
#[derive(Debug, Clone, Copy, Default)]
pub struct EditorFallbackHandler;

impl InputFallbackHandler for EditorFallbackHandler {
    fn handle_unmatched(&self, key: KeyEvent, app: &mut AppState) -> FallbackResult {
        let mode_id = app.current_mode();

        // Check if we're in Insert mode
        if *mode_id == EditorMode::INSERT_ID {
            // Try to extract a printable character
            if let Some(_ch) = key_to_char(&key) {
                // TODO: Actually insert the character into the buffer
                // For now, just return Handled to verify the wiring works
                //
                // Full implementation would be:
                // if let Some(buffer_id) = app.active_buffer {
                //     if let Some(buffer) = app.kernel.buffers.get(buffer_id) {
                //         buffer.write().insert(&_ch.to_string());
                //     }
                // }

                return FallbackResult::Handled;
            }

            // Non-printable key in Insert mode - ignore it
            return FallbackResult::Ignored;
        }

        // In Normal mode, unmatched keys should beep
        if *mode_id == EditorMode::NORMAL_ID {
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
        let result = handler.handle_unmatched(key, &mut app);

        assert_eq!(result, FallbackResult::Beep);
    }

    #[test]
    fn test_insert_mode_handles_char() {
        let handler = EditorFallbackHandler;
        let mut app = create_app_insert();

        let key = KeyEvent::new(KeyCode::Char('a'));
        let result = handler.handle_unmatched(key, &mut app);

        assert_eq!(result, FallbackResult::Handled);
    }

    #[test]
    fn test_insert_mode_handles_uppercase() {
        let handler = EditorFallbackHandler;
        let mut app = create_app_insert();

        let key = KeyEvent::with_modifiers(KeyCode::Char('A'), Modifiers::SHIFT);
        let result = handler.handle_unmatched(key, &mut app);

        assert_eq!(result, FallbackResult::Handled);
    }

    #[test]
    fn test_insert_mode_ignores_ctrl_char() {
        let handler = EditorFallbackHandler;
        let mut app = create_app_insert();

        let key = KeyEvent::with_modifiers(KeyCode::Char('c'), Modifiers::CTRL);
        let result = handler.handle_unmatched(key, &mut app);

        assert_eq!(result, FallbackResult::Ignored);
    }

    #[test]
    fn test_insert_mode_handles_tab() {
        let handler = EditorFallbackHandler;
        let mut app = create_app_insert();

        let key = KeyEvent::new(KeyCode::Tab);
        let result = handler.handle_unmatched(key, &mut app);

        assert_eq!(result, FallbackResult::Handled);
    }

    #[test]
    fn test_insert_mode_handles_enter() {
        let handler = EditorFallbackHandler;
        let mut app = create_app_insert();

        let key = KeyEvent::new(KeyCode::Enter);
        let result = handler.handle_unmatched(key, &mut app);

        assert_eq!(result, FallbackResult::Handled);
    }

    #[test]
    fn test_insert_mode_ignores_special_keys() {
        let handler = EditorFallbackHandler;
        let mut app = create_app_insert();

        // Function keys, arrow keys, etc. should be ignored
        let key = KeyEvent::new(KeyCode::F(1));
        let result = handler.handle_unmatched(key, &mut app);

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
