//! Settings menu layer - full-screen settings overlay

use crate::{
    frame::FrameBuffer,
    highlight::{ColorMode, Theme},
    screen::{Layer, LayerBounds, z_order},
    settings_menu::SettingsMenuState,
};

/// Layer wrapper for settings menu state
pub struct SettingsMenuLayer<'a> {
    state: &'a SettingsMenuState,
    _screen_width: u16,
    _screen_height: u16,
}

impl<'a> SettingsMenuLayer<'a> {
    /// Create a new settings menu layer
    #[must_use]
    pub const fn new(state: &'a SettingsMenuState, screen_width: u16, screen_height: u16) -> Self {
        Self {
            state,
            _screen_width: screen_width,
            _screen_height: screen_height,
        }
    }
}

impl Layer for SettingsMenuLayer<'_> {
    fn z_order(&self) -> u8 {
        z_order::SETTINGS_MENU
    }

    fn is_visible(&self) -> bool {
        self.state.visible
    }

    fn bounds(&self) -> LayerBounds {
        LayerBounds::new(
            self.state.layout.x,
            self.state.layout.y,
            self.state.layout.width,
            self.state.layout.height,
        )
    }

    fn render_to_buffer(&self, buffer: &mut FrameBuffer, theme: &Theme, color_mode: ColorMode) {
        let lines = self.state.render(theme, color_mode);
        for (line, x, y) in lines {
            buffer.write_str(x, y, &line, &crate::highlight::Style::default());
        }
    }

    #[allow(clippy::cast_possible_truncation)]
    fn cursor_position(&self) -> Option<(u16, u16)> {
        let cursor_y = self.state.layout.y
            + 2
            + self
                .state
                .selected_index
                .saturating_sub(self.state.scroll_offset) as u16;
        let cursor_x = self.state.layout.x + 2;
        Some((cursor_x, cursor_y))
    }
}
