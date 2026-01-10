//! Mouse event handler methods for the Runtime
//!
//! This module contains mouse click and scroll event handling, including:
//! - Window focus based on click position
//! - Cursor positioning from screen coordinates
//! - Viewport scrolling with mouse wheel

use crate::{
    event::{MouseButton, MouseEvent, MouseEventKind},
    screen::window::Anchor,
};

use super::Runtime;

impl Runtime {
    /// Handle mouse events (clicks, scrolls, etc.)
    ///
    /// Dispatches to specific handlers based on event kind:
    /// - Left click: focus window and move cursor
    /// - Scroll up/down: scroll viewport
    pub(crate) fn handle_mouse_event(&mut self, event: MouseEvent) {
        tracing::debug!("Mouse event: {:?} at ({}, {})", event.kind, event.column, event.row);

        match event.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                self.handle_mouse_click(event.column, event.row);
            }
            MouseEventKind::ScrollUp => {
                self.handle_mouse_scroll(-3); // Scroll up = move viewport up
            }
            MouseEventKind::ScrollDown => {
                self.handle_mouse_scroll(3); // Scroll down = move viewport down
            }
            // Ignore other mouse events for now (drag, right-click, etc.)
            _ => {}
        }
    }

    /// Handle mouse click at screen position
    ///
    /// - Finds the window under the click
    /// - Focuses the window if different from current
    /// - Moves cursor to clicked buffer position
    fn handle_mouse_click(&mut self, screen_x: u16, screen_y: u16) {
        // Find which window was clicked
        let Some(window_id) = self.screen.window_at_position(screen_x, screen_y) else {
            tracing::debug!("Click outside any window at ({}, {})", screen_x, screen_y);
            return;
        };

        // Get window info before mutating
        let (buffer_id, buffer_line_count) = {
            let Some(window) = self.screen.window(window_id) else {
                return;
            };
            let Some(buffer_id) = window.buffer_id() else {
                return; // Window doesn't have a buffer
            };
            let buffer_line_count = self.buffers.get(&buffer_id).map_or(0, |b| b.contents.len());
            (buffer_id, buffer_line_count)
        };

        // Note: has_signs is false for now since sign data isn't readily accessible
        // This may cause slight positioning inaccuracy when signs are present
        let has_signs = false;

        // Translate screen position to buffer position
        let buffer_pos = {
            let Some(window) = self.screen.window(window_id) else {
                return;
            };
            window.screen_to_buffer_position(screen_x, screen_y, buffer_line_count, has_signs)
        };

        let Some(buffer_pos) = buffer_pos else {
            tracing::debug!("Click on scrollbar or invalid position");
            return;
        };

        // Focus window if different from current active window
        let current_active = self.screen.active_window_id();
        if current_active != Some(window_id) {
            tracing::debug!("Focusing window {} (was {:?})", window_id, current_active);
            self.screen.set_active_window(window_id);
            // Screen is now the source of truth for active buffer
            // set_active_window already updates which window is active
        }

        // Move cursor to clicked position
        if let Some(buffer) = self.buffers.get_mut(&buffer_id) {
            // Clamp x to line length
            let line_len = buffer
                .contents
                .get(buffer_pos.y as usize)
                .map_or(0, |l| l.inner.len());
            #[allow(clippy::cast_possible_truncation)]
            let clamped_x = if line_len == 0 {
                0
            } else {
                buffer_pos.x.min((line_len.saturating_sub(1)) as u16)
            };

            buffer.cur.x = clamped_x;
            buffer.cur.y = buffer_pos.y;

            tracing::debug!(
                "Cursor moved to ({}, {}) in buffer {}",
                buffer.cur.x,
                buffer.cur.y,
                buffer_id
            );
        }

        self.request_render();
    }

    /// Handle mouse scroll by delta lines
    ///
    /// Positive delta = scroll down (content moves up)
    /// Negative delta = scroll up (content moves down)
    fn handle_mouse_scroll(&mut self, delta: i16) {
        let Some(active_window_id) = self.screen.active_window_id() else {
            return;
        };

        let Some(window) = self.screen.window_mut(active_window_id) else {
            return;
        };

        let Some(buffer_id) = window.buffer_id() else {
            return;
        };

        let buffer_line_count = self.buffers.get(&buffer_id).map_or(0, |b| b.contents.len());

        // Get current buffer anchor (scroll position)
        let current_anchor = window.buffer_anchor().unwrap_or(Anchor { x: 0, y: 0 });

        // Calculate new scroll position
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let new_y = if delta > 0 {
            // Scroll down: increase anchor.y
            let max_scroll = buffer_line_count.saturating_sub(window.height as usize);
            (current_anchor.y as usize + delta as usize).min(max_scroll) as u16
        } else {
            // Scroll up: decrease anchor.y
            current_anchor.y.saturating_sub((-delta) as u16)
        };

        // Update buffer anchor
        window.set_buffer_anchor(Anchor {
            x: current_anchor.x,
            y: new_y,
        });

        // Adjust cursor if it goes off-screen
        if let Some(buffer) = self.buffers.get_mut(&buffer_id) {
            let viewport_top = new_y;
            let viewport_bottom = new_y + window.height.saturating_sub(1);

            if buffer.cur.y < viewport_top {
                // Cursor above viewport - move to top
                buffer.cur.y = viewport_top;
            } else if buffer.cur.y > viewport_bottom {
                // Cursor below viewport - move to bottom
                buffer.cur.y = viewport_bottom;
            }

            // Ensure cursor x is still valid for the new line
            let line_len = buffer
                .contents
                .get(buffer.cur.y as usize)
                .map_or(0, |l| l.inner.len());
            #[allow(clippy::cast_possible_truncation)]
            if line_len == 0 {
                buffer.cur.x = 0;
            } else {
                buffer.cur.x = buffer.cur.x.min((line_len.saturating_sub(1)) as u16);
            }
        }

        self.request_render();
    }
}
