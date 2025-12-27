//! Completion popup window
//!
//! Plugin window that renders the completion dropdown.
//! Reads from the lock-free cache for non-blocking rendering.
//! Also renders ghost text inline at cursor position.

use std::sync::Arc;

use reovim_core::{
    frame::FrameBuffer,
    highlight::{Style, Theme},
    plugin::{EditorContext, PluginStateRegistry, PluginWindow, Rect, WindowConfig},
    sys::style::Color,
};

use crate::state::SharedCompletionManager;

/// Plugin window for completion popup
///
/// Reads from the lock-free cache for rendering.
pub struct CompletionPluginWindow {
    manager: Arc<SharedCompletionManager>,
}

impl CompletionPluginWindow {
    /// Create a new completion window
    #[must_use]
    pub fn new(manager: Arc<SharedCompletionManager>) -> Self {
        Self { manager }
    }
}

impl PluginWindow for CompletionPluginWindow {
    #[allow(clippy::cast_possible_truncation)]
    fn window_config(
        &self,
        _state: &Arc<PluginStateRegistry>,
        ctx: &EditorContext,
    ) -> Option<WindowConfig> {
        let snapshot = self.manager.snapshot();

        if !snapshot.active || snapshot.items.is_empty() {
            return None;
        }

        // Transform buffer coordinates to screen coordinates
        // Account for: window anchor, line number gutter, and scroll offset
        let display_row = (snapshot.cursor_row as u16).saturating_sub(ctx.active_window_scroll_y);
        let screen_y = ctx.active_window_anchor_y + display_row;
        let screen_x = ctx
            .active_window_anchor_x
            .saturating_add(ctx.active_window_gutter_width)
            .saturating_add(snapshot.word_start_col as u16);

        let max_items = 10.min(snapshot.items.len());
        let popup_width = snapshot
            .items
            .iter()
            .take(max_items)
            .map(|i| i.label.len())
            .max()
            .map_or(12, |w| (w + 2).min(40)) as u16;

        // Use EditorContext::dropdown() for proper bounds clamping
        let (popup_x, popup_y, _, popup_height) =
            ctx.dropdown(screen_x, screen_y, popup_width, max_items as u16);

        Some(WindowConfig {
            bounds: Rect::new(popup_x, popup_y, popup_width, popup_height),
            z_order: 200, // Completion dropdown
            visible: true,
        })
    }

    #[allow(clippy::cast_possible_truncation)]
    fn render(
        &self,
        _state: &Arc<PluginStateRegistry>,
        ctx: &EditorContext,
        buffer: &mut FrameBuffer,
        bounds: Rect,
        theme: &Theme,
    ) {
        let snapshot = self.manager.snapshot();

        if !snapshot.active || snapshot.items.is_empty() {
            return;
        }

        let popup_x = bounds.x;
        let popup_y = bounds.y;
        let popup_width = bounds.width;
        let max_items = bounds.height as usize;

        // Render the popup menu
        for (idx, item) in snapshot.items.iter().take(max_items).enumerate() {
            let is_selected = idx == snapshot.selected_index;
            let style = if is_selected {
                &theme.popup.selected
            } else {
                &theme.popup.normal
            };

            let row = popup_y + idx as u16;
            if row >= ctx.screen_height.saturating_sub(1) {
                break;
            }

            buffer.put_char(popup_x, row, ' ', style);
            let label_chars: Vec<char> = item.label.chars().collect();
            for (i, &ch) in label_chars
                .iter()
                .take(popup_width as usize - 2)
                .enumerate()
            {
                buffer.put_char(popup_x + 1 + i as u16, row, ch, style);
            }
            for i in label_chars.len().min(popup_width as usize - 2)..popup_width as usize - 1 {
                buffer.put_char(popup_x + 1 + i as u16, row, ' ', style);
            }
            buffer.put_char(popup_x + popup_width - 1, row, ' ', style);
        }

        // Render ghost text inline at cursor position
        self.render_ghost_text(&snapshot, ctx, buffer, bounds);
    }
}

impl CompletionPluginWindow {
    /// Render ghost text (remaining completion text) inline at cursor position
    #[allow(clippy::cast_possible_truncation)]
    fn render_ghost_text(
        &self,
        snapshot: &crate::cache::CompletionSnapshot,
        ctx: &EditorContext,
        buffer: &mut FrameBuffer,
        _bounds: Rect,
    ) {
        // Get the selected item
        let Some(item) = snapshot.selected_item() else {
            return;
        };

        // Calculate ghost text: the part of insert_text not yet typed
        // insert_text is the full text, prefix is what's already typed
        let ghost_text = if item.insert_text.len() > snapshot.prefix.len()
            && item.insert_text.starts_with(&snapshot.prefix)
        {
            &item.insert_text[snapshot.prefix.len()..]
        } else if item.label.len() > snapshot.prefix.len()
            && item
                .label
                .to_lowercase()
                .starts_with(&snapshot.prefix.to_lowercase())
        {
            // Fallback to label if insert_text doesn't match
            &item.label[snapshot.prefix.len()..]
        } else {
            return;
        };

        if ghost_text.is_empty() {
            return;
        }

        // Ghost text style: dim grey
        let ghost_style = Style::new().fg(Color::DarkGrey).dim();

        // Calculate screen coordinates using EditorContext
        // Ghost text renders at cursor position (after the typed prefix)
        let ghost_y = ctx.cursor_screen_y();
        let ghost_x = ctx.cursor_screen_x();

        // Don't render past screen width
        let max_width = ctx.screen_width.saturating_sub(ghost_x);

        for (i, ch) in ghost_text.chars().take(max_width as usize).enumerate() {
            buffer.put_char(ghost_x + i as u16, ghost_y, ch, &ghost_style);
        }
    }
}
