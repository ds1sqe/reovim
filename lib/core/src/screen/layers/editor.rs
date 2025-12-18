//! Editor layer - main text editing windows

use std::collections::BTreeMap;

use crate::{
    buffer::Buffer,
    folding::FoldManager,
    frame::FrameBuffer,
    highlight::{ColorMode, HighlightStore, Style, Theme},
    indent::IndentAnalyzer,
    screen::{Layer, LayerBounds, layout::LayoutManager, window::Window, z_order},
};

/// Layer wrapper for editor windows
pub struct EditorLayer<'a> {
    windows: &'a mut [Window],
    buffers: &'a BTreeMap<usize, Buffer>,
    highlight_store: &'a HighlightStore,
    fold_manager: &'a FoldManager,
    indent_analyzer: &'a IndentAnalyzer,
    layout: &'a LayoutManager,
    screen_width: u16,
    screen_height: u16,
}

impl<'a> EditorLayer<'a> {
    /// Create a new editor layer
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub const fn new(
        windows: &'a mut [Window],
        buffers: &'a BTreeMap<usize, Buffer>,
        highlight_store: &'a HighlightStore,
        fold_manager: &'a FoldManager,
        indent_analyzer: &'a IndentAnalyzer,
        layout: &'a LayoutManager,
        screen_width: u16,
        screen_height: u16,
    ) -> Self {
        Self {
            windows,
            buffers,
            highlight_store,
            fold_manager,
            indent_analyzer,
            layout,
            screen_width,
            screen_height,
        }
    }
}

impl Layer for EditorLayer<'_> {
    fn z_order(&self) -> u8 {
        z_order::EDITOR
    }

    fn is_visible(&self) -> bool {
        true // Editor is always visible
    }

    fn bounds(&self) -> LayerBounds {
        // Editor takes remaining space after explorer
        LayerBounds::full_screen(self.screen_width, self.screen_height)
    }

    #[allow(clippy::cast_possible_truncation)]
    fn render_to_buffer(&self, buffer: &mut FrameBuffer, theme: &Theme, color_mode: ColorMode) {
        for win in self.windows.iter() {
            if let Some(buf) = self.buffers.get(&win.buffer_id) {
                // Get fold state for this buffer
                let fold_state = self.fold_manager.get(buf.id);

                // Render each line
                let lines = win.render(
                    buf,
                    self.highlight_store,
                    color_mode,
                    theme,
                    fold_state,
                    self.indent_analyzer,
                );

                for (row_offset, line) in lines.iter().enumerate() {
                    let y = win.anchor.y + row_offset as u16;
                    if y < self.screen_height {
                        // The window render returns pre-styled ANSI strings
                        // Write them to the buffer (simplified - ideally parse styles)
                        buffer.write_str(win.anchor.x, y, line, &Style::default());
                    }
                }
            }
        }
    }

    fn cursor_position(&self) -> Option<(u16, u16)> {
        // Only return cursor if editor is focused (not explorer)
        if self.layout.is_explorer_focused() {
            return None;
        }

        // Find the active window and buffer
        for win in self.windows.iter() {
            if let Some(buf) = self.buffers.get(&win.buffer_id) {
                let gutter_width = win.line_number_width(buf.contents.len());
                let cursor_x = win.anchor.x + gutter_width + buf.cur.x;
                let cursor_y = win.anchor.y + buf.cur.y.saturating_sub(win.buffer_anchor.y);
                return Some((cursor_x, cursor_y));
            }
        }
        None
    }
}
