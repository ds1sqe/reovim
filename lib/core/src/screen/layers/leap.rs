//! Leap labels layer - jump target markers

use crate::{
    frame::FrameBuffer,
    highlight::{ColorMode, Theme},
    leap::LeapState,
    screen::{Layer, LayerBounds, z_order},
};

/// Layer wrapper for leap labels
pub struct LeapLayer<'a> {
    state: &'a LeapState,
    /// Window anchor X (after gutter)
    window_x: u16,
    /// Window anchor Y
    window_y: u16,
    /// Scroll offset
    scroll_offset: u16,
    screen_width: u16,
    screen_height: u16,
}

impl<'a> LeapLayer<'a> {
    /// Create a new leap layer
    #[must_use]
    pub const fn new(
        state: &'a LeapState,
        window_x: u16,
        window_y: u16,
        scroll_offset: u16,
        screen_width: u16,
        screen_height: u16,
    ) -> Self {
        Self {
            state,
            window_x,
            window_y,
            scroll_offset,
            screen_width,
            screen_height,
        }
    }
}

impl Layer for LeapLayer<'_> {
    fn z_order(&self) -> u8 {
        z_order::LEAP
    }

    fn is_visible(&self) -> bool {
        self.state.is_showing_labels()
    }

    fn bounds(&self) -> LayerBounds {
        // Leap labels can appear anywhere in the editor area
        LayerBounds::full_screen(self.screen_width, self.screen_height)
    }

    #[allow(clippy::cast_possible_truncation)]
    fn render_to_buffer(&self, buffer: &mut FrameBuffer, theme: &Theme, _color_mode: ColorMode) {
        let label_style = &theme.leap.label;

        for target in &self.state.matches {
            // Convert buffer position to screen position
            let screen_y = target.line.saturating_sub(self.scroll_offset);
            let y = self.window_y + screen_y;

            if y < self.screen_height {
                let x = self.window_x + target.col;
                if x < self.screen_width {
                    // Draw the label character(s)
                    for (i, ch) in target.label.chars().enumerate() {
                        if x + (i as u16) < self.screen_width {
                            buffer.put_char(x + (i as u16), y, ch, label_style);
                        }
                    }
                }
            }
        }
    }

    fn cursor_position(&self) -> Option<(u16, u16)> {
        // Leap labels don't own the cursor
        None
    }
}
