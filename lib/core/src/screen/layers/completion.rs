//! Completion popup layer

use crate::{
    completion::CompletionState,
    frame::FrameBuffer,
    highlight::{ColorMode, Theme},
    screen::{Layer, LayerBounds, z_order},
};

/// Layer wrapper for completion popup
pub struct CompletionLayer<'a> {
    state: &'a CompletionState,
    cursor_x: u16,
    cursor_y: u16,
    screen_width: u16,
    screen_height: u16,
}

impl<'a> CompletionLayer<'a> {
    /// Create a new completion layer
    #[must_use]
    pub const fn new(
        state: &'a CompletionState,
        cursor_x: u16,
        cursor_y: u16,
        screen_width: u16,
        screen_height: u16,
    ) -> Self {
        Self {
            state,
            cursor_x,
            cursor_y,
            screen_width,
            screen_height,
        }
    }
}

impl Layer for CompletionLayer<'_> {
    fn z_order(&self) -> u8 {
        z_order::COMPLETION
    }

    fn is_visible(&self) -> bool {
        self.state.is_visible() && !self.state.items.is_empty()
    }

    #[allow(clippy::cast_possible_truncation)]
    fn bounds(&self) -> LayerBounds {
        let items = &self.state.items;
        if items.is_empty() {
            return LayerBounds::default();
        }

        let max_visible = 10;
        let popup_height = items.len().min(max_visible) as u16 + 2; // +2 for border
        let max_width = items
            .iter()
            .map(|item| item.label.len())
            .max()
            .unwrap_or(10) as u16
            + 4;
        let popup_width = max_width.min(40);

        // Position popup below cursor, or above if not enough space
        let space_below = self.screen_height.saturating_sub(self.cursor_y + 1);
        let y = if space_below >= popup_height {
            self.cursor_y + 1
        } else {
            self.cursor_y.saturating_sub(popup_height)
        };

        let x = self
            .cursor_x
            .min(self.screen_width.saturating_sub(popup_width));

        LayerBounds::new(x, y, popup_width, popup_height)
    }

    #[allow(clippy::cast_possible_truncation)]
    fn render_to_buffer(&self, buffer: &mut FrameBuffer, theme: &Theme, _color_mode: ColorMode) {
        let items = &self.state.items;
        if items.is_empty() {
            return;
        }

        let bounds = self.bounds();
        let x = bounds.x;
        let y = bounds.y;
        let width = bounds.width;
        let height = bounds.height;

        let border_style = theme.popup.border.clone();
        let normal_style = theme.popup.normal.clone();
        let selected_style = theme.popup.selected.clone();

        // Top border
        buffer.put_char(x, y, '┌', &border_style);
        for i in 1..width - 1 {
            buffer.put_char(x + i, y, '─', &border_style);
        }
        buffer.put_char(x + width - 1, y, '┐', &border_style);

        // Items
        let max_visible = (height - 2) as usize;
        let scroll_offset = self.state.selected_index.saturating_sub(max_visible - 1);

        for (idx, item) in items
            .iter()
            .skip(scroll_offset)
            .take(max_visible)
            .enumerate()
        {
            let row_y = y + 1 + idx as u16;
            let is_selected = scroll_offset + idx == self.state.selected_index;

            buffer.put_char(x, row_y, '│', &border_style);

            let style = if is_selected {
                &selected_style
            } else {
                &normal_style
            };
            let label = &item.label;
            let max_len = (width - 2) as usize;
            let display = if label.len() > max_len {
                format!("{}..", &label[..max_len.saturating_sub(2)])
            } else {
                format!("{label:max_len$}")
            };

            buffer.write_str(x + 1, row_y, &display, style);
            buffer.put_char(x + width - 1, row_y, '│', &border_style);
        }

        // Bottom border
        let bottom_y = y + height - 1;
        buffer.put_char(x, bottom_y, '└', &border_style);
        for i in 1..width - 1 {
            buffer.put_char(x + i, bottom_y, '─', &border_style);
        }
        buffer.put_char(x + width - 1, bottom_y, '┘', &border_style);
    }

    fn cursor_position(&self) -> Option<(u16, u16)> {
        // Completion popup doesn't own the cursor
        None
    }
}
