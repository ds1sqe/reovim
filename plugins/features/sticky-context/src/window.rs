//! Sticky context window renderer

use std::sync::Arc;

use reovim_core::{
    frame::FrameBuffer,
    highlight::Theme,
    plugin::{EditorContext, PluginStateRegistry, PluginWindow, Rect, WindowConfig},
};

use crate::state::StickyContextState;

/// Sticky context overlay window
pub struct StickyContextWindow {
    state: Arc<StickyContextState>,
}

impl StickyContextWindow {
    /// Create a new sticky context window
    pub const fn new(state: Arc<StickyContextState>) -> Self {
        Self { state }
    }
}

impl PluginWindow for StickyContextWindow {
    fn window_config(
        &self,
        _state: &Arc<PluginStateRegistry>,
        ctx: &EditorContext,
    ) -> Option<WindowConfig> {
        // Only show when feature is enabled and editor is focused
        if !self.state.is_enabled() {
            return None;
        }

        if !ctx.is_editor_focused() {
            return None;
        }

        // Position: overlay at top of active window
        // Height: will be determined during render based on context depth
        // Start with max possible height
        #[allow(clippy::cast_possible_truncation)] // max_lines is always small (1-5)
        let max_height = self.state.max_lines() as u16 + u16::from(self.state.show_separator());

        Some(WindowConfig {
            bounds: Rect::new(
                ctx.active_window_anchor_x,
                ctx.active_window_anchor_y,
                ctx.screen_width
                    .saturating_sub(ctx.active_window_anchor_x)
                    .saturating_sub(ctx.right_offset),
                max_height,
            ),
            z_order: 125, // Between editor (100) and sidebar (150)
            visible: true,
        })
    }

    fn render(
        &self,
        _state: &Arc<PluginStateRegistry>,
        ctx: &EditorContext,
        buffer: &mut FrameBuffer,
        bounds: Rect,
        theme: &Theme,
    ) {
        let buffer_id = ctx.active_buffer_id;

        // Get cached context from ViewportContextUpdated event
        let Some(cached) = self.state.get_cached_context() else {
            return; // No cached context yet
        };

        // Verify context is for current buffer
        if cached.buffer_id != buffer_id {
            return; // Cached context is for a different buffer
        }

        // Get the hierarchy from cached context
        let Some(ref hierarchy) = cached.context else {
            return; // No context at this position
        };

        if hierarchy.items.is_empty() {
            return; // Nothing to display
        }

        // Take first N items (outermost scopes)
        let max_lines = self.state.max_lines();
        let items_to_show = hierarchy.items.iter().take(max_lines);

        // Render each context line
        let mut y = bounds.y;
        for item in items_to_show {
            if y >= bounds.y + bounds.height {
                break; // Ran out of space
            }

            let mut x = bounds.x;

            // Format: "line_num │ scope text"
            let line_num = format!("{:>4}", item.start_line + 1);
            let separator = " │ ";

            // Render line number
            for ch in line_num.chars() {
                if x >= bounds.x + bounds.width {
                    break;
                }
                buffer.put_char(x, y, ch, &theme.gutter.line_number);
                x += 1;
            }

            // Render separator
            for ch in separator.chars() {
                if x >= bounds.x + bounds.width {
                    break;
                }
                buffer.put_char(x, y, ch, &theme.gutter.inactive_line_number);
                x += 1;
            }

            // Render scope text
            for ch in item.text.chars() {
                if x >= bounds.x + bounds.width {
                    break; // Truncate at viewport edge
                }
                buffer.put_char(x, y, ch, &theme.gutter.inactive_line_number);
                x += 1;
            }

            y += 1;
        }

        // Render separator line
        if self.state.show_separator() && y < bounds.y + bounds.height {
            let separator_char = '─';
            for x in bounds.x..bounds.x + bounds.width {
                buffer.put_char(x, y, separator_char, &theme.gutter.inactive_line_number);
            }
        }
    }
}
