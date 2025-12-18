//! Explorer layer - file explorer sidebar

use crate::{
    explorer::{ExplorerState, render_explorer},
    frame::FrameBuffer,
    highlight::{ColorMode, Style, Theme},
    screen::{Layer, LayerBounds, layout::LayoutManager, z_order},
};

/// Layer wrapper for explorer sidebar
pub struct ExplorerLayer<'a> {
    state: &'a ExplorerState,
    layout: &'a LayoutManager,
}

impl<'a> ExplorerLayer<'a> {
    /// Create a new explorer layer
    #[must_use]
    pub const fn new(state: &'a ExplorerState, layout: &'a LayoutManager) -> Self {
        Self { state, layout }
    }
}

impl Layer for ExplorerLayer<'_> {
    fn z_order(&self) -> u8 {
        z_order::EXPLORER
    }

    fn is_visible(&self) -> bool {
        self.layout.is_explorer_visible()
    }

    fn bounds(&self) -> LayerBounds {
        self.layout
            .explorer_layout()
            .map_or_else(LayerBounds::default, |layout| {
                LayerBounds::new(layout.anchor.x, layout.anchor.y, layout.width, layout.height)
            })
    }

    #[allow(clippy::cast_possible_truncation)]
    fn render_to_buffer(&self, buffer: &mut FrameBuffer, theme: &Theme, color_mode: ColorMode) {
        let Some(layout) = self.layout.explorer_layout() else {
            return;
        };

        let lines = render_explorer(self.state, layout.width, layout.height, theme, color_mode);

        for (row_offset, line) in lines.iter().enumerate() {
            let y = layout.anchor.y + row_offset as u16;
            // The render_explorer function returns pre-styled strings with ANSI codes
            // For buffer rendering, we need to write the plain text
            // This is a simplified version - ideally we'd parse the styled output
            buffer.write_str(layout.anchor.x, y, line, &Style::default());
        }
    }

    #[allow(clippy::cast_possible_truncation)]
    fn cursor_position(&self) -> Option<(u16, u16)> {
        if self.layout.is_explorer_focused()
            && let Some(layout) = self.layout.explorer_layout()
        {
            let cursor_y = self
                .state
                .cursor_index
                .saturating_sub(self.state.scroll_offset);
            return Some((layout.anchor.x, layout.anchor.y + cursor_y as u16));
        }
        None
    }
}
