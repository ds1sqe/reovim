//! Tab line display component
//!
//! Renders the tab bar at the top of the screen when multiple tabs are open.

use crate::{
    component::{DisplayComponent, RenderContext},
    frame::FrameBuffer,
    screen::{LayerBounds, tab::TabInfo},
};

/// Tab line display component
///
/// Displays open tabs at the top of the screen.
/// Only visible when there are 2 or more tabs.
#[derive(Debug)]
pub struct TabLineComponent<'a> {
    /// Tab information (id, label, `is_active`)
    pub tabs: &'a [TabInfo],
    /// Active tab index
    pub active_index: usize,
}

impl<'a> TabLineComponent<'a> {
    /// Create a new tab line component
    #[must_use]
    pub const fn new(tabs: &'a [TabInfo], active_index: usize) -> Self {
        Self { tabs, active_index }
    }
}

impl DisplayComponent for TabLineComponent<'_> {
    fn component_id(&self) -> &'static str {
        "tab_line"
    }

    fn is_visible(&self, _context: &RenderContext<'_>) -> bool {
        // Only visible when there are multiple tabs
        self.tabs.len() > 1
    }

    #[allow(clippy::cast_possible_truncation)]
    fn render_to_frame(&self, buffer: &mut FrameBuffer, context: &RenderContext<'_>) {
        if !self.is_visible(context) {
            return;
        }

        let theme = context.theme;
        let mut x = 0u16;

        for (idx, tab) in self.tabs.iter().enumerate() {
            let is_active = idx == self.active_index;
            let style = if is_active {
                &theme.tab.active
            } else {
                &theme.tab.inactive
            };

            let label = format!(" {} ", tab.label);
            buffer.write_str(x, 0, &label, style);
            x += label.len() as u16;
        }

        // Fill rest of tab line with fill style
        let fill_style = theme.tab.fill.clone();
        for col in x..context.screen_width {
            buffer.put_char(col, 0, ' ', &fill_style);
        }
    }

    fn bounds(&self, context: &RenderContext<'_>) -> LayerBounds {
        if self.is_visible(context) {
            LayerBounds {
                x: 0,
                y: 0,
                width: context.screen_width,
                height: 1,
            }
        } else {
            LayerBounds::default()
        }
    }
}
