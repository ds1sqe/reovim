//! Character operation handlers for the event loop.
//!
//! Handles pending character operations (f, F, t, T, r) that require
//! a character argument to complete.

use {
    reovim_driver_input::{KeyCode, KeyEvent},
    reovim_kernel::api::v1::{Motion, MotionEngine, Position},
};

use super::EventLoop;
use crate::server::{
    AppState,
    app::{LastFind, PendingCharOp},
};

impl<F: reovim_driver_input::InputFallbackHandler<AppState>> EventLoop<F> {
    /// Handle a key event when in pending-char state (f/F/t/T or r pending).
    ///
    /// The key provides the character argument for the pending operation.
    /// Escape cancels the wait without executing any operation.
    pub(super) fn handle_pending_char(&mut self, key: KeyEvent) {
        // Take the pending-char state
        let pending = self
            .app
            .take_pending_char()
            .expect("pending_char should be Some");

        // Handle escape - cancel pending operation
        if key.code == KeyCode::Escape {
            self.app.clear_pending_keys();
            return;
        }

        // Extract character from key event
        let KeyCode::Char(c) = key.code else {
            // Non-character keys cancel pending operation
            self.app.clear_pending_keys();
            return;
        };

        match pending {
            PendingCharOp::FindForward { start: _ }
            | PendingCharOp::FindBackward { start: _ }
            | PendingCharOp::TillForward { start: _ }
            | PendingCharOp::TillBackward { start: _ } => {
                self.execute_find_char(c, &pending);
            }
            PendingCharOp::ReplaceChar { count } => {
                self.execute_replace_char(c, count);
            }
        }

        self.app.clear_pending_keys();
    }

    /// Execute a find-char motion with the given character.
    pub(super) fn execute_find_char(&mut self, c: char, pending: &PendingCharOp) {
        let Some(find_type) = pending.find_type() else {
            return; // Not a find-char operation
        };

        // Build the find-char motion
        let motion = Motion::FindChar {
            char: c,
            direction: find_type.direction(),
            till: find_type.is_till(),
        };

        // Get active buffer for motion calculation
        let Some(buffer_id) = self.app.active_buffer else {
            self.set_error("No active buffer");
            return;
        };

        let Some(buffer_arc) = self.app.kernel.buffers.get(buffer_id) else {
            self.set_error("Buffer not found");
            return;
        };

        // Calculate motion target
        let buffer = buffer_arc.read();
        let target = MotionEngine::calculate(&buffer, buffer.cursor(), motion, 1);
        drop(buffer);

        // Apply motion if target found
        if let Some(pos) = target {
            buffer_arc.write().set_position(pos);

            // Update last_find for ; and , repeat
            self.app.last_find = Some(LastFind::new(c, find_type));
        }
        // Note: If target is None (char not found), cursor doesn't move,
        // and last_find is NOT updated (per Vim behavior)
    }

    /// Execute a replace-char operation with the given character.
    pub(super) fn execute_replace_char(&mut self, c: char, count: usize) {
        // Get active buffer
        let Some(buffer_id) = self.app.active_buffer else {
            self.set_error("No active buffer");
            return;
        };

        let Some(buffer_arc) = self.app.kernel.buffers.get(buffer_id) else {
            self.set_error("Buffer not found");
            return;
        };

        let mut buffer = buffer_arc.write();
        let cursor_pos = buffer.cursor().position;

        // Replace count characters with c
        let line = buffer.lines().get(cursor_pos.line).map(String::from);
        if let Some(line) = line {
            let start_col = cursor_pos.column;
            let end_col = (start_col + count).min(line.len());

            if start_col < line.len() {
                // Build replacement string
                let replacement: String = std::iter::repeat_n(c, end_col - start_col).collect();

                // Delete old characters and insert new
                let start = Position::new(cursor_pos.line, start_col);
                let end = Position::new(cursor_pos.line, end_col);
                buffer.delete_range(start, end);
                buffer.insert_at(start, &replacement);

                // Cursor stays at start position (Vim behavior)
                buffer.set_position(start);
            }
        }
    }
}
