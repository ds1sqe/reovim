//! Window separator rendering for the editor
//!
//! This module contains the window separator rendering logic, drawing vertical
//! and horizontal separators between split windows.

use crate::{frame::FrameBuffer, highlight::Theme};

use super::Screen;

impl Screen {
    /// Render window separators to frame buffer
    ///
    /// Draws vertical and horizontal separators between adjacent windows
    /// when multiple windows are visible.
    pub(super) fn render_window_separators_to_buffer(
        &self,
        buffer: &mut FrameBuffer,
        theme: &Theme,
    ) {
        if self.windows.len() <= 1 {
            return;
        }

        let sep_style = &theme.window.separator;

        for i in 0..self.windows.len() {
            for j in (i + 1)..self.windows.len() {
                let win_a = &self.windows[i];
                let win_b = &self.windows[j];

                // Vertical separator
                if win_a.anchor.x + win_a.width == win_b.anchor.x {
                    let sep_x = win_b.anchor.x.saturating_sub(1);
                    let start_y = win_a.anchor.y.max(win_b.anchor.y);
                    let end_y = (win_a.anchor.y + win_a.height).min(win_b.anchor.y + win_b.height);

                    for y in start_y..end_y {
                        buffer.put_char(sep_x, y, '│', sep_style);
                    }
                }

                // Horizontal separator
                if win_a.anchor.y + win_a.height == win_b.anchor.y {
                    let sep_y = win_b.anchor.y.saturating_sub(1);
                    let start_x = win_a.anchor.x.max(win_b.anchor.x);
                    let end_x = (win_a.anchor.x + win_a.width).min(win_b.anchor.x + win_b.width);

                    for x in start_x..end_x {
                        buffer.put_char(x, sep_y, '─', sep_style);
                    }
                }
            }
        }
    }
}
