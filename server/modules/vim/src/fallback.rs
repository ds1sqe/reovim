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
use reovim_types_text::{Edit, Position};

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
    #[cfg_attr(coverage_nightly, coverage(off))]
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
                        Position::new(cursor_before.line + 1, 0)
                    } else {
                        Position::new(
                            cursor_before.line,
                            cursor_before.column + 1,
                        )
                    };

                    // Update cursor position
                    ctx.set_cursor_position(cursor_after);

                    // Accumulate edit for batched undo tracking
                    // (consecutive inserts become single undo node)
                    let edit = Edit::insert(cursor_before, &text);
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
#[path = "fallback_tests.rs"]
mod tests;
