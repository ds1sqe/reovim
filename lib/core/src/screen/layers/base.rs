//! Base layer - tab line and status line

use crate::{
    buffer::Buffer,
    command_line::CommandLine,
    frame::FrameBuffer,
    highlight::{ColorMode, Style, Theme},
    modd::{EditMode, Focus, ModeState},
    screen::{Layer, LayerBounds, tab::TabManager, z_order},
};

/// Get the appropriate mode style from the theme based on current mode state
fn get_mode_style(mode: &ModeState, theme: &Theme) -> Style {
    match (&mode.focus, &mode.edit_mode) {
        (Focus::Editor, EditMode::Insert(_)) => theme.statusline.mode.insert.clone(),
        (Focus::Editor, EditMode::Visual(_)) => theme.statusline.mode.visual.clone(),
        (Focus::Explorer, _) => theme.statusline.mode.explorer.clone(),
        // Normal mode, telescope, settings menu all use normal style
        (Focus::Editor, EditMode::Normal) | (Focus::Telescope | Focus::SettingsMenu, _) => {
            theme.statusline.mode.normal.clone()
        }
    }
}

/// Layer wrapper for base UI elements (tab line, status line)
pub struct BaseLayer<'a> {
    mode: &'a ModeState,
    cmd_line: &'a CommandLine,
    pending_keys: &'a str,
    _last_command: &'a str,
    current_buffer: Option<&'a Buffer>,
    tab_manager: &'a TabManager,
    screen_width: u16,
    screen_height: u16,
}

impl<'a> BaseLayer<'a> {
    /// Create a new base layer
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub const fn new(
        mode: &'a ModeState,
        cmd_line: &'a CommandLine,
        pending_keys: &'a str,
        last_command: &'a str,
        current_buffer: Option<&'a Buffer>,
        tab_manager: &'a TabManager,
        screen_width: u16,
        screen_height: u16,
    ) -> Self {
        Self {
            mode,
            cmd_line,
            pending_keys,
            _last_command: last_command,
            current_buffer,
            tab_manager,
            screen_width,
            screen_height,
        }
    }
}

impl Layer for BaseLayer<'_> {
    fn z_order(&self) -> u8 {
        z_order::BASE
    }

    fn is_visible(&self) -> bool {
        true // Base layer is always visible
    }

    fn bounds(&self) -> LayerBounds {
        LayerBounds::full_screen(self.screen_width, self.screen_height)
    }

    fn render_to_buffer(&self, buffer: &mut FrameBuffer, theme: &Theme, color_mode: ColorMode) {
        // Render tab line if multiple tabs
        if self.tab_manager.tab_count() > 1 {
            self.render_tab_line(buffer, theme, color_mode);
        }

        // Render status line or command line at bottom
        let status_y = self.screen_height - 1;
        if self.mode.is_command() {
            self.render_command_line(buffer, status_y);
        } else {
            self.render_status_line(buffer, status_y, theme, color_mode);
        }
    }

    #[allow(clippy::cast_possible_truncation)]
    fn cursor_position(&self) -> Option<(u16, u16)> {
        // In command mode, cursor is in the command line
        if self.mode.is_command() {
            let y = self.screen_height - 1;
            let x = 1 + self.cmd_line.cursor as u16; // +1 for ':'
            return Some((x, y));
        }
        None
    }
}

impl BaseLayer<'_> {
    #[allow(clippy::cast_possible_truncation)]
    fn render_tab_line(&self, buffer: &mut FrameBuffer, theme: &Theme, _color_mode: ColorMode) {
        let tabs = self.tab_manager.tab_info();
        let active_idx = self.tab_manager.active_tab_index();

        let mut x = 0u16;
        for (idx, tab) in tabs.iter().enumerate() {
            let is_active = idx == active_idx;
            let style = if is_active {
                &theme.tab.active
            } else {
                &theme.tab.inactive
            };

            let label = format!(" {} ", tab.label);
            buffer.write_str(x, 0, &label, style);
            x += label.len() as u16;
        }

        // Fill rest of tab line
        let fill_style = theme.tab.fill.clone();
        for col in x..self.screen_width {
            buffer.put_char(col, 0, ' ', &fill_style);
        }
    }

    #[allow(clippy::cast_possible_truncation)]
    fn render_status_line(
        &self,
        buffer: &mut FrameBuffer,
        y: u16,
        theme: &Theme,
        _color_mode: ColorMode,
    ) {
        // Mode indicator
        let mode_display = self.mode.display_string();
        let mode_style = get_mode_style(self.mode, theme);
        let mode_text = format!(" {mode_display} ");
        buffer.write_str(0, y, &mode_text, &mode_style);

        let mut x = mode_text.len() as u16;

        // Pending keys
        if !self.pending_keys.is_empty() {
            let pending = format!(" {} ", self.pending_keys);
            buffer.write_str(x, y, &pending, &theme.statusline.background);
            x += pending.len() as u16;
        }

        // Fill middle
        let right_content = self.current_buffer.map_or_else(String::new, |buf| {
            let name = buf.file_path.as_deref().unwrap_or("[No Name]");
            let modified = if buf.modified { "[+]" } else { "" };
            format!("{name}{modified} Ln {}, Col {} ", buf.cur.y + 1, buf.cur.x + 1)
        });

        let right_start = self.screen_width.saturating_sub(right_content.len() as u16);
        let fill_style = theme.statusline.background.clone();

        for col in x..right_start {
            buffer.put_char(col, y, ' ', &fill_style);
        }

        // Right side content
        buffer.write_str(right_start, y, &right_content, &theme.statusline.background);
    }

    #[allow(clippy::cast_possible_truncation)]
    fn render_command_line(&self, buffer: &mut FrameBuffer, y: u16) {
        let style = Style::default();

        // Draw ':'
        buffer.put_char(0, y, ':', &style);

        // Draw command content
        let content = &self.cmd_line.input;
        buffer.write_str(1, y, content, &style);

        // Fill rest
        let used = 1 + content.len() as u16;
        for col in used..self.screen_width {
            buffer.put_char(col, y, ' ', &style);
        }
    }
}
