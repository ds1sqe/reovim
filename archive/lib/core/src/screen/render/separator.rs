//! Window separator rendering for the editor
//!
//! This module contains the window separator rendering logic, drawing vertical
//! and horizontal separators between split windows.

use crate::{frame::FrameBuffer, highlight::Theme};

use super::super::Screen;

impl Screen {
    /// Render window separators to frame buffer
    ///
    /// Draws vertical and horizontal separators between adjacent windows
    /// when multiple windows are visible.
    pub(in crate::screen) fn render_window_separators_to_buffer(
        &self,
        buffer: &mut FrameBuffer,
        theme: &Theme,
    ) {
        let windows: Vec<_> = self.window_store.iter().collect();
        if windows.len() <= 1 {
            return;
        }

        let sep_style = &theme.window.separator;

        for i in 0..windows.len() {
            for j in (i + 1)..windows.len() {
                let win_a = windows[i];
                let win_b = windows[j];

                // Vertical separator
                if win_a.bounds.x + win_a.bounds.width == win_b.bounds.x {
                    let sep_x = win_b.bounds.x.saturating_sub(1);
                    let start_y = win_a.bounds.y.max(win_b.bounds.y);
                    let end_y = (win_a.bounds.y + win_a.bounds.height)
                        .min(win_b.bounds.y + win_b.bounds.height);

                    for y in start_y..end_y {
                        buffer.put_char(sep_x, y, '│', sep_style);
                    }
                }

                // Horizontal separator
                if win_a.bounds.y + win_a.bounds.height == win_b.bounds.y {
                    let sep_y = win_b.bounds.y.saturating_sub(1);
                    let start_x = win_a.bounds.x.max(win_b.bounds.x);
                    let end_x = (win_a.bounds.x + win_a.bounds.width)
                        .min(win_b.bounds.x + win_b.bounds.width);

                    for x in start_x..end_x {
                        buffer.put_char(x, sep_y, '─', sep_style);
                    }
                }
            }
        }
    }
}
