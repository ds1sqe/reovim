//! Tab line rendering for the editor
//!
//! This module contains the tab line rendering logic, displaying multiple tabs
//! when more than one tab is open.

use crate::{
    frame::FrameBuffer,
    highlight::{ColorMode, Theme},
};

use super::Screen;

impl Screen {
    /// Render tab line to frame buffer
    ///
    /// Displays tabs when multiple tabs are open. Shows active tab with
    /// highlighted style and inactive tabs with dimmed style.
    pub(super) fn render_tab_line_to_buffer(
        &self,
        buffer: &mut FrameBuffer,
        _color_mode: ColorMode,
        theme: &Theme,
    ) {
        let tabs = self.tab_manager.tab_info();
        if tabs.len() <= 1 {
            return;
        }

        let mut x = 0u16;
        for tab in &tabs {
            let style = if tab.is_active {
                &theme.tab.active
            } else {
                &theme.tab.inactive
            };

            let label = format!(" {} ", tab.label);
            for ch in label.chars() {
                if x < buffer.width() {
                    buffer.put_char(x, 0, ch, style);
                    x += 1;
                }
            }
        }

        // Fill rest with tab fill style
        let fill_style = &theme.tab.fill;
        while x < buffer.width() {
            buffer.put_char(x, 0, ' ', fill_style);
            x += 1;
        }
    }
}
