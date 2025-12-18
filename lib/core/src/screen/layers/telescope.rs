//! Telescope layer - full-screen fuzzy finder overlay

use crate::{
    frame::FrameBuffer,
    highlight::{ColorMode, Style, Theme},
    screen::{Layer, LayerBounds, z_order},
    telescope::TelescopeState,
};

/// Layer wrapper for telescope state
pub struct TelescopeLayer<'a> {
    state: &'a TelescopeState,
    screen_width: u16,
    screen_height: u16,
}

impl<'a> TelescopeLayer<'a> {
    /// Create a new telescope layer
    #[must_use]
    pub const fn new(state: &'a TelescopeState, screen_width: u16, screen_height: u16) -> Self {
        Self {
            state,
            screen_width,
            screen_height,
        }
    }

    /// Write a styled string to the buffer
    fn write_styled(buffer: &mut FrameBuffer, x: u16, y: u16, text: &str, style: &Style) {
        buffer.write_str(x, y, text, style);
    }

    /// Fill a horizontal line with a character
    fn fill_char(buffer: &mut FrameBuffer, x: u16, y: u16, ch: char, count: u16, style: &Style) {
        for i in 0..count {
            buffer.put_char(x + i, y, ch, style);
        }
    }
}

impl Layer for TelescopeLayer<'_> {
    fn z_order(&self) -> u8 {
        z_order::TELESCOPE
    }

    fn is_visible(&self) -> bool {
        self.state.is_visible()
    }

    fn bounds(&self) -> LayerBounds {
        // Telescope is full screen
        LayerBounds::full_screen(self.screen_width, self.screen_height)
    }

    #[allow(clippy::cast_possible_truncation)]
    #[allow(clippy::too_many_lines)]
    fn render_to_buffer(&self, buffer: &mut FrameBuffer, theme: &Theme, _color_mode: ColorMode) {
        let layout = &self.state.layout;
        let x = layout.x;
        let y = layout.y;
        let width = layout.width;
        let height = layout.height;
        let preview_width = layout.preview_width;

        // Calculate total width including preview
        let total_width = preview_width.map_or(width, |pw| width + 1 + pw);

        let border_style = theme.telescope.border.clone();
        let normal_style = theme.telescope.normal.clone();
        let selected_style = theme.telescope.selected.clone();
        let prompt_style = theme.telescope.prompt.clone();

        // Top border with title
        let title = if self.state.title.is_empty() {
            format!(" {} ", self.state.picker_name)
        } else {
            format!(" {} ", self.state.title)
        };
        let title_len = title.len();

        // Top-left corner
        buffer.put_char(x, y, '╭', &border_style);
        Self::write_styled(buffer, x + 1, y, &title, &border_style);
        let remaining = (total_width as usize).saturating_sub(title_len + 2);
        Self::fill_char(buffer, x + 1 + title_len as u16, y, '─', remaining as u16, &border_style);
        buffer.put_char(x + total_width - 1, y, '╮', &border_style);

        // Results area (items)
        let items_height = height.saturating_sub(4);
        let visible_items = self.state.visible_items();

        for row in 0..items_height {
            let screen_y = y + 1 + row;

            // Left border
            buffer.put_char(x, screen_y, '│', &border_style);

            let idx = row as usize;
            if idx < visible_items.len() {
                let item = &visible_items[idx];
                let absolute_idx = self.state.scroll_offset + idx;
                let is_selected = absolute_idx == self.state.selected_index;

                let style = if is_selected {
                    &selected_style
                } else {
                    &normal_style
                };

                let display = &item.display;
                let max_display_len = (width as usize).saturating_sub(2);
                let display_str = if display.len() > max_display_len {
                    format!("{}..", &display[..max_display_len.saturating_sub(2)])
                } else {
                    format!("{display:max_display_len$}")
                };

                Self::write_styled(buffer, x + 1, screen_y, &display_str, style);
            } else {
                // Empty row - fill with spaces
                Self::fill_char(buffer, x + 1, screen_y, ' ', width - 2, &normal_style);
            }

            // Right border of results
            buffer.put_char(x + width - 1, screen_y, '│', &border_style);

            // Preview panel (if enabled)
            if let Some(pw) = preview_width {
                let preview_content = self.state.preview.as_ref();
                let line_idx = row as usize;

                if let Some(preview) = preview_content
                    && line_idx < preview.lines.len()
                {
                    let preview_line = &preview.lines[line_idx];
                    let is_highlight_line = preview.highlight_line == Some(line_idx);

                    let style = if is_highlight_line {
                        &theme.telescope.preview_highlight
                    } else {
                        &theme.telescope.preview
                    };

                    let max_len = (pw as usize).saturating_sub(1);
                    let line_str = if preview_line.len() > max_len {
                        format!("{}..", &preview_line[..max_len.saturating_sub(2)])
                    } else {
                        format!("{preview_line:max_len$}")
                    };

                    Self::write_styled(buffer, x + width, screen_y, &line_str, style);
                } else {
                    // Empty preview line
                    Self::fill_char(
                        buffer,
                        x + width,
                        screen_y,
                        ' ',
                        pw - 1,
                        &theme.telescope.preview,
                    );
                }

                // Right border of preview
                buffer.put_char(x + total_width - 1, screen_y, '│', &border_style);
            }
        }

        // Separator line before prompt
        let sep_y = y + height - 3;
        buffer.put_char(x, sep_y, '├', &border_style);
        Self::fill_char(buffer, x + 1, sep_y, '─', total_width - 2, &border_style);
        buffer.put_char(x + total_width - 1, sep_y, '┤', &border_style);

        // Prompt line
        let prompt_y = y + height - 2;
        buffer.put_char(x, prompt_y, '│', &border_style);

        let prompt = &self.state.prompt;
        let query = &self.state.query;
        Self::write_styled(buffer, x + 1, prompt_y, prompt, &prompt_style);
        Self::write_styled(buffer, x + 1 + prompt.len() as u16, prompt_y, query, &normal_style);

        // Fill remaining space
        let used = prompt.len() + query.len();
        let remaining_space = (total_width as usize).saturating_sub(used + 2);
        Self::fill_char(
            buffer,
            x + 1 + used as u16,
            prompt_y,
            ' ',
            remaining_space as u16,
            &normal_style,
        );
        buffer.put_char(x + total_width - 1, prompt_y, '│', &border_style);

        // Bottom border with count
        let bottom_y = y + height - 1;
        buffer.put_char(x, bottom_y, '╰', &border_style);

        let count_str = format!(
            " {}/{} ",
            self.state.items.len().min(self.state.selected_index + 1),
            self.state.items.len()
        );
        let count_len = count_str.len();
        let left_fill = (total_width as usize).saturating_sub(count_len + 2);
        Self::fill_char(buffer, x + 1, bottom_y, '─', left_fill as u16, &border_style);
        Self::write_styled(buffer, x + 1 + left_fill as u16, bottom_y, &count_str, &border_style);
        buffer.put_char(x + total_width - 1, bottom_y, '╯', &border_style);
    }

    #[allow(clippy::cast_possible_truncation)]
    fn cursor_position(&self) -> Option<(u16, u16)> {
        let prompt_len = self.state.prompt.len() as u16;
        let cursor_x = self.state.layout.x + 1 + prompt_len + self.state.cursor_pos as u16;
        let cursor_y = self.state.layout.y + self.state.layout.height - 2;
        Some((cursor_x, cursor_y))
    }
}
