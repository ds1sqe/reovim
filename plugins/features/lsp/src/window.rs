//! LSP hover popup window.
//!
//! Plugin window that renders hover documentation in a floating popup.
//! Reads from the lock-free `HoverCache` for non-blocking rendering.

use std::sync::Arc;

use reovim_core::{
    frame::FrameBuffer,
    highlight::Theme,
    plugin::{EditorContext, PluginStateRegistry, PluginWindow, Rect, WindowConfig},
};

use crate::SharedLspManager;

/// Plugin window for LSP hover popup.
///
/// Displays hover documentation in a floating window near the cursor.
pub struct HoverPluginWindow {
    manager: Arc<SharedLspManager>,
}

impl HoverPluginWindow {
    /// Create a new hover window.
    #[must_use]
    #[allow(clippy::missing_const_for_fn)] // Arc is not const
    pub fn new(manager: Arc<SharedLspManager>) -> Self {
        Self { manager }
    }

    /// Calculate popup bounds based on anchor position and content size.
    #[allow(clippy::cast_possible_truncation, clippy::unused_self)]
    fn calculate_bounds(
        &self,
        snapshot: &crate::hover::HoverSnapshot,
        ctx: &EditorContext,
    ) -> Rect {
        // Calculate content dimensions
        let content_width = snapshot.max_line_width().min(60) as u16 + 4; // +4 for borders and padding
        let content_height = snapshot.line_count().min(15) as u16 + 2; // +2 for borders

        // Position popup below the anchor, or above if not enough space
        let anchor_x = snapshot.anchor_col as u16;
        let anchor_y = snapshot.anchor_row as u16;

        // Try to position below cursor
        let popup_y = if anchor_y + 1 + content_height < ctx.screen_height {
            anchor_y + 1
        } else if anchor_y >= content_height {
            // Position above if not enough space below
            anchor_y.saturating_sub(content_height)
        } else {
            // Center vertically if neither works
            (ctx.screen_height.saturating_sub(content_height)) / 2
        };

        // Clamp X position to screen bounds
        let popup_x = anchor_x.min(ctx.screen_width.saturating_sub(content_width));

        Rect::new(popup_x, popup_y, content_width, content_height)
    }

    /// Draw a box border around the popup.
    #[allow(clippy::unused_self)]
    fn draw_border(&self, buffer: &mut FrameBuffer, bounds: Rect, theme: &Theme) {
        let style = &theme.popup.border;
        let x = bounds.x;
        let y = bounds.y;
        let w = bounds.width;
        let h = bounds.height;

        // Top border
        buffer.put_char(x, y, '┌', style);
        for i in 1..w.saturating_sub(1) {
            buffer.put_char(x + i, y, '─', style);
        }
        buffer.put_char(x + w.saturating_sub(1), y, '┐', style);

        // Side borders
        for i in 1..h.saturating_sub(1) {
            buffer.put_char(x, y + i, '│', style);
            buffer.put_char(x + w.saturating_sub(1), y + i, '│', style);
        }

        // Bottom border
        buffer.put_char(x, y + h.saturating_sub(1), '└', style);
        for i in 1..w.saturating_sub(1) {
            buffer.put_char(x + i, y + h.saturating_sub(1), '─', style);
        }
        buffer.put_char(x + w.saturating_sub(1), y + h.saturating_sub(1), '┘', style);
    }

    /// Render content inside the popup (excluding borders).
    #[allow(clippy::cast_possible_truncation, clippy::unused_self)]
    fn render_content(
        &self,
        snapshot: &crate::hover::HoverSnapshot,
        buffer: &mut FrameBuffer,
        bounds: Rect,
        theme: &Theme,
    ) {
        let style = &theme.popup.normal;
        let content_x = bounds.x + 1; // After left border
        let content_y = bounds.y + 1; // After top border
        let content_width = bounds.width.saturating_sub(2) as usize;
        let content_height = bounds.height.saturating_sub(2) as usize;

        // Fill background
        for row in 0..content_height {
            for col in 0..content_width {
                buffer.put_char(content_x + col as u16, content_y + row as u16, ' ', style);
            }
        }

        // Render text lines
        for (row, line) in snapshot.lines().take(content_height).enumerate() {
            let y = content_y + row as u16;

            // Render each character, respecting content width
            for (col, ch) in line.chars().take(content_width).enumerate() {
                buffer.put_char(content_x + col as u16, y, ch, style);
            }
        }
    }
}

impl PluginWindow for HoverPluginWindow {
    fn window_config(
        &self,
        _state: &Arc<PluginStateRegistry>,
        ctx: &EditorContext,
    ) -> Option<WindowConfig> {
        // Get hover snapshot from manager
        let snapshot = self.manager.with(|m| m.hover_cache.load())?;

        // Only show if we have content
        if snapshot.content.is_empty() {
            return None;
        }

        let bounds = self.calculate_bounds(&snapshot, ctx);

        Some(WindowConfig {
            bounds,
            z_order: 300, // Floating window level
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
        // Get hover snapshot from manager
        let Some(snapshot) = self.manager.with(|m| m.hover_cache.load()) else {
            return;
        };

        if snapshot.content.is_empty() {
            return;
        }

        // Ensure bounds are valid
        if bounds.is_empty() || bounds.x >= ctx.screen_width || bounds.y >= ctx.screen_height {
            return;
        }

        // Draw border
        self.draw_border(buffer, bounds, theme);

        // Draw content
        self.render_content(&snapshot, buffer, bounds, theme);
    }
}
