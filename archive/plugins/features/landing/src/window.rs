//! Landing page plugin window
//!
//! Renders the landing page with animated ASCII lion when no files are open.

use std::sync::Arc;

use reovim_core::{
    frame::FrameBuffer,
    highlight::Theme,
    plugin::{EditorContext, PluginStateRegistry, PluginWindow, Rect, WindowConfig},
};

use crate::state::LandingState;

/// Plugin window for landing page display
///
/// Shows the animated lion mascot and help hints when no files are open.
pub struct LandingPluginWindow;

impl LandingPluginWindow {
    /// Create a new landing window
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Default for LandingPluginWindow {
    fn default() -> Self {
        Self::new()
    }
}

impl PluginWindow for LandingPluginWindow {
    fn window_config(
        &self,
        state: &Arc<PluginStateRegistry>,
        ctx: &EditorContext,
    ) -> Option<WindowConfig> {
        // Only show landing page when no buffers are open
        if ctx.buffer_count > 0 {
            return None;
        }

        // Check if we have landing state, create if needed
        let has_state = state.with::<LandingState, _, _>(|_| true).unwrap_or(false);
        if !has_state {
            // Initialize state on first access
            let landing = LandingState::new(ctx.screen_width, ctx.screen_height);
            state.register(landing);
        }

        // Full screen for landing page, low z-order
        Some(WindowConfig {
            bounds: Rect::new(0, 0, ctx.screen_width, ctx.screen_height.saturating_sub(1)), // Leave room for statusline
            z_order: 50, // Below editor windows
            visible: true,
        })
    }

    #[allow(clippy::cast_possible_truncation)]
    fn render(
        &self,
        state: &Arc<PluginStateRegistry>,
        _ctx: &EditorContext,
        buffer: &mut FrameBuffer,
        bounds: Rect,
        theme: &Theme,
    ) {
        // Get or create landing state
        let content = state
            .with::<LandingState, _, _>(|landing| landing.generate(bounds.width, bounds.height))
            .unwrap_or_default();

        let style = &theme.base.default;

        // Render content line by line
        for (row_idx, line) in content.lines().enumerate() {
            let row = bounds.y + row_idx as u16;
            if row >= bounds.y + bounds.height {
                break;
            }

            let mut col = bounds.x;
            for ch in line.chars() {
                if col >= bounds.x + bounds.width {
                    break;
                }
                buffer.put_char(col, row, ch, style);
                col += 1;
            }

            // Fill rest of line with spaces
            while col < bounds.x + bounds.width {
                buffer.put_char(col, row, ' ', style);
                col += 1;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_landing_window_creation() {
        // Verify that LandingPluginWindow can be created
        let window = LandingPluginWindow::new();
        assert!(std::mem::size_of_val(&window) == 0); // Unit struct
    }
}
