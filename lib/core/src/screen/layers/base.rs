//! Base layer - tab line and status line
//!
//! Uses `UIComponent` trait implementations for rendering.

use crate::{
    buffer::Buffer,
    command_line::CommandLine,
    component::{RenderContext, StatusLineComponent, TabLineComponent},
    frame::FrameBuffer,
    highlight::{ColorMode, Style, Theme},
    modd::ModeState,
    screen::{Layer, LayerBounds, tab::TabManager, z_order},
    ui_component::UIComponent,
};

/// Layer wrapper for base UI elements (tab line, status line)
pub struct BaseLayer<'a> {
    mode: &'a ModeState,
    cmd_line: &'a CommandLine,
    pending_keys: &'a str,
    last_command: &'a str,
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
            last_command,
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
        // Create render context
        let tab_offset = u16::from(self.tab_manager.tab_count() > 1);
        let context = RenderContext::new(self.screen_width, self.screen_height, theme, color_mode)
            .with_tab_offset(tab_offset);

        // Render tab line using TabLineComponent
        let tabs = self.tab_manager.tab_info();
        let tab_component = TabLineComponent::new(&tabs, self.tab_manager.active_tab_index());
        tab_component.render_to_frame(buffer, &context);

        // Render status line or command line at bottom
        if self.mode.is_command() {
            self.render_command_line(buffer, context.status_line_row());
        } else {
            // Use StatusLineComponent
            let status_component = StatusLineComponent::new(
                self.mode,
                self.current_buffer,
                self.pending_keys,
                self.last_command,
            );
            status_component.render_to_frame(buffer, &context);
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
