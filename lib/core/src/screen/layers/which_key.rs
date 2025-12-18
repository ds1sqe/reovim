//! Which-key layer - hint panel overlay

use crate::{
    frame::FrameBuffer,
    highlight::{ColorMode, Theme},
    screen::{Layer, LayerBounds, WhichKeyPanel, z_order},
};

/// Layer wrapper for which-key panel
pub struct WhichKeyLayer<'a> {
    panel: &'a WhichKeyPanel,
    screen_width: u16,
    screen_height: u16,
}

impl<'a> WhichKeyLayer<'a> {
    /// Create a new which-key layer
    #[must_use]
    pub const fn new(panel: &'a WhichKeyPanel, screen_width: u16, screen_height: u16) -> Self {
        Self {
            panel,
            screen_width,
            screen_height,
        }
    }
}

impl Layer for WhichKeyLayer<'_> {
    fn z_order(&self) -> u8 {
        z_order::WHICH_KEY
    }

    fn is_visible(&self) -> bool {
        self.panel.visible
    }

    fn bounds(&self) -> LayerBounds {
        // Which-key panel is positioned at bottom-right
        // The actual bounds are computed when rendering
        LayerBounds::new(0, 0, self.screen_width, self.screen_height)
    }

    fn render_to_buffer(&self, buffer: &mut FrameBuffer, theme: &Theme, color_mode: ColorMode) {
        let panel_lines =
            self.panel
                .render(self.screen_width, self.screen_height, color_mode, theme);
        for (line, x, y) in panel_lines {
            buffer.write_str(x, y, &line, &crate::highlight::Style::default());
        }
    }

    fn cursor_position(&self) -> Option<(u16, u16)> {
        // Which-key panel doesn't own the cursor
        None
    }
}
