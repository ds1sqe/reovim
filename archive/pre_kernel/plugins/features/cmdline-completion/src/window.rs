//! Command-line completion popup window
//!
//! Implements `PluginWindow` trait for rendering the wildmenu-style popup.

use std::sync::Arc;

use reovim_core::{
    frame::FrameBuffer,
    highlight::{Style, Theme},
    plugin::{EditorContext, PluginStateRegistry, PluginWindow, Rect, WindowConfig},
    sys::style::Color,
};

use crate::cache::{CmdlineCompletionCache, CmdlineCompletionKind};

/// Get icon color for completion kind
fn kind_color(kind: CmdlineCompletionKind) -> Color {
    match kind {
        CmdlineCompletionKind::Command => Color::Magenta,
        CmdlineCompletionKind::File => Color::Cyan,
        CmdlineCompletionKind::Directory => Color::Yellow,
        CmdlineCompletionKind::Option => Color::Green,
        CmdlineCompletionKind::Subcommand => Color::Blue,
    }
}

/// Plugin window for command-line completion popup
pub struct CmdlineCompletionWindow {
    cache: Arc<CmdlineCompletionCache>,
}

impl CmdlineCompletionWindow {
    /// Create a new completion window
    #[must_use]
    pub fn new(cache: Arc<CmdlineCompletionCache>) -> Self {
        Self { cache }
    }
}

impl PluginWindow for CmdlineCompletionWindow {
    #[allow(clippy::cast_possible_truncation)]
    fn window_config(
        &self,
        _state: &Arc<PluginStateRegistry>,
        ctx: &EditorContext,
    ) -> Option<WindowConfig> {
        let snapshot = self.cache.load();

        if !snapshot.active || snapshot.items.is_empty() {
            return None;
        }

        // Calculate popup dimensions
        let max_items = 10.min(snapshot.items.len());
        let popup_height = max_items as u16;

        // Calculate width based on longest item
        // Format: " icon label    description "
        let icon_width = 2_usize; // icon + space
        let max_label = snapshot
            .items
            .iter()
            .take(max_items)
            .map(|i| i.label.len())
            .max()
            .unwrap_or(10);
        let max_desc = snapshot
            .items
            .iter()
            .take(max_items)
            .map(|i| i.description.len())
            .max()
            .unwrap_or(20);

        // Total: padding + icon + label + gap + description + padding
        let popup_width =
            (1 + icon_width + max_label.min(30) + 2 + max_desc.min(25) + 1).min(60) as u16;

        // Position: above the command line (status line is at bottom row)
        // Command line is at screen_height - 1
        let popup_y = ctx
            .screen_height
            .saturating_sub(1)
            .saturating_sub(popup_height);

        // X position: align with where completion started + 1 (for colon prefix)
        let popup_x =
            (1 + snapshot.replace_start as u16).min(ctx.screen_width.saturating_sub(popup_width));

        Some(WindowConfig {
            bounds: Rect::new(popup_x, popup_y, popup_width, popup_height),
            z_order: 200, // Dropdown level
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
        let snapshot = self.cache.load();

        if !snapshot.active || snapshot.items.is_empty() {
            return;
        }

        let popup_x = bounds.x;
        let popup_y = bounds.y;
        let popup_width = bounds.width;
        let max_items = bounds.height as usize;

        // Calculate column widths
        let icon_col_width = 2_u16; // icon + space
        let desc_col_width = snapshot
            .items
            .iter()
            .take(max_items)
            .map(|i| i.description.len())
            .max()
            .unwrap_or(20)
            .min(25) as u16;
        let label_col_width = popup_width
            .saturating_sub(1) // left padding
            .saturating_sub(icon_col_width)
            .saturating_sub(2) // gap before description
            .saturating_sub(desc_col_width)
            .saturating_sub(1); // right padding

        // Render each item
        for (idx, item) in snapshot.items.iter().take(max_items).enumerate() {
            let is_selected = idx == snapshot.selected_index;
            let base_style = if is_selected {
                &theme.popup.selected
            } else {
                &theme.popup.normal
            };

            let row = popup_y + idx as u16;
            if row >= ctx.screen_height.saturating_sub(1) {
                break;
            }

            let mut col = popup_x;

            // Left padding
            buffer.put_char(col, row, ' ', base_style);
            col += 1;

            // Icon (colored)
            let icon_fg = kind_color(item.kind);
            let icon_style = if is_selected {
                base_style.clone().fg(icon_fg)
            } else {
                Style::new()
                    .fg(icon_fg)
                    .bg(base_style.bg.unwrap_or(Color::Black))
            };
            for ch in item.icon.chars() {
                buffer.put_char(col, row, ch, &icon_style);
                col += 1;
            }
            // Pad to icon column width
            while col < popup_x + 1 + icon_col_width {
                buffer.put_char(col, row, ' ', base_style);
                col += 1;
            }

            // Label
            let label_chars: Vec<char> = item.label.chars().collect();
            for ch in label_chars.iter().take(label_col_width as usize) {
                buffer.put_char(col, row, *ch, base_style);
                col += 1;
            }
            // Pad label column
            let label_drawn = label_chars.len().min(label_col_width as usize);
            for _ in label_drawn..label_col_width as usize {
                buffer.put_char(col, row, ' ', base_style);
                col += 1;
            }

            // Gap before description
            buffer.put_char(col, row, ' ', base_style);
            col += 1;
            buffer.put_char(col, row, ' ', base_style);
            col += 1;

            // Description (dimmed)
            let desc_style = if is_selected {
                base_style.clone().dim()
            } else {
                Style::new()
                    .fg(Color::DarkGrey)
                    .bg(base_style.bg.unwrap_or(Color::Black))
            };
            for ch in item.description.chars().take(desc_col_width as usize) {
                buffer.put_char(col, row, ch, &desc_style);
                col += 1;
            }
            // Pad description column
            let desc_drawn = item.description.len().min(desc_col_width as usize);
            for _ in desc_drawn..desc_col_width as usize {
                buffer.put_char(col, row, ' ', base_style);
                col += 1;
            }

            // Right padding
            buffer.put_char(col, row, ' ', base_style);
        }
    }
}
