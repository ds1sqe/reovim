//! Completion popup window
//!
//! Plugin window that renders the completion dropdown.
//! Reads from the lock-free cache for non-blocking rendering.
//! Also renders ghost text inline at cursor position.

use std::{collections::HashSet, sync::Arc};

use reovim_core::{
    completion::CompletionKind,
    frame::FrameBuffer,
    highlight::{Style, Theme},
    plugin::{EditorContext, PluginStateRegistry, PluginWindow, Rect, WindowConfig},
    sys::style::Color,
};

use crate::state::SharedCompletionManager;

/// Get short abbreviation for completion kind (2-3 chars)
fn kind_abbrev(kind: CompletionKind) -> &'static str {
    match kind {
        CompletionKind::Text => "txt",
        CompletionKind::Method => "fn",
        CompletionKind::Function => "fn",
        CompletionKind::Constructor => "new",
        CompletionKind::Field => "fld",
        CompletionKind::Variable => "var",
        CompletionKind::Class => "cls",
        CompletionKind::Interface => "int",
        CompletionKind::Module => "mod",
        CompletionKind::Property => "prp",
        CompletionKind::Unit => "unt",
        CompletionKind::Value => "val",
        CompletionKind::Enum => "enm",
        CompletionKind::Keyword => "kw",
        CompletionKind::Snippet => "snp",
        CompletionKind::Color => "clr",
        CompletionKind::File => "fil",
        CompletionKind::Reference => "ref",
        CompletionKind::Folder => "dir",
        CompletionKind::EnumMember => "enm",
        CompletionKind::Constant => "cst",
        CompletionKind::Struct => "st",
        CompletionKind::Event => "evt",
        CompletionKind::Operator => "op",
        CompletionKind::TypeParameter => "typ",
    }
}

/// Get color for completion kind
fn kind_color(kind: CompletionKind) -> Color {
    match kind {
        CompletionKind::Function | CompletionKind::Method | CompletionKind::Constructor => {
            Color::Magenta
        }
        CompletionKind::Variable | CompletionKind::Field | CompletionKind::Property => Color::Cyan,
        CompletionKind::Module | CompletionKind::Class | CompletionKind::Interface => Color::Yellow,
        CompletionKind::Struct | CompletionKind::Enum | CompletionKind::EnumMember => Color::Green,
        CompletionKind::Keyword => Color::Blue,
        CompletionKind::Snippet => Color::Red,
        CompletionKind::Constant => Color::Cyan,
        _ => Color::White,
    }
}

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

        // Calculate widths for each column
        // Format: " [kind] label    [source] "
        let kind_width = 4_usize; // "fn " or "mod" + space
        let max_label_width = snapshot
            .items
            .iter()
            .take(max_items)
            .map(|i| i.label.len())
            .max()
            .unwrap_or(8);
        let max_source_width = snapshot
            .items
            .iter()
            .take(max_items)
            .map(|i| i.source.len())
            .max()
            .unwrap_or(6);

        // Total: space + kind + label + gap + source + space
        let popup_width =
            (1 + kind_width + max_label_width.min(30) + 1 + max_source_width.min(12) + 1).min(60)
                as u16;

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

        // Calculate column widths for rendering
        let kind_col_width = 4_u16; // "fn " + space
        let source_col_width = snapshot
            .items
            .iter()
            .take(max_items)
            .map(|i| i.source.len())
            .max()
            .unwrap_or(6)
            .min(12) as u16;
        let label_col_width = popup_width
            .saturating_sub(1) // left padding
            .saturating_sub(kind_col_width)
            .saturating_sub(1) // gap before source
            .saturating_sub(source_col_width)
            .saturating_sub(1); // right padding

        // Render the popup menu
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

            // Kind abbreviation (colored)
            let kind_abbr = kind_abbrev(item.kind);
            let kind_fg = kind_color(item.kind);
            let kind_style = if is_selected {
                base_style.clone().fg(kind_fg)
            } else {
                Style::new()
                    .fg(kind_fg)
                    .bg(base_style.bg.unwrap_or(Color::Black))
            };
            for ch in kind_abbr.chars() {
                buffer.put_char(col, row, ch, &kind_style);
                col += 1;
            }
            // Pad kind column
            for _ in kind_abbr.len()..kind_col_width as usize {
                buffer.put_char(col, row, ' ', base_style);
                col += 1;
            }

            // Label with match highlighting
            let matched_set: HashSet<u32> = item.match_indices.iter().copied().collect();
            let label_chars: Vec<char> = item.label.chars().collect();
            for (i, &ch) in label_chars
                .iter()
                .take(label_col_width as usize)
                .enumerate()
            {
                let char_style = if matched_set.contains(&(i as u32)) {
                    let match_fg_color = theme.popup.match_fg.fg.unwrap_or(Color::Yellow);
                    if is_selected {
                        base_style.clone().fg(match_fg_color)
                    } else {
                        let bg_color = base_style.bg.unwrap_or(Color::Black);
                        theme.popup.match_fg.clone().bg(bg_color)
                    }
                } else {
                    base_style.clone()
                };
                buffer.put_char(col, row, ch, &char_style);
                col += 1;
            }
            // Pad label column
            for _ in label_chars.len().min(label_col_width as usize)..label_col_width as usize {
                buffer.put_char(col, row, ' ', base_style);
                col += 1;
            }

            // Gap before source
            buffer.put_char(col, row, ' ', base_style);
            col += 1;

            // Source (dimmed)
            let source_style = if is_selected {
                base_style.clone().dim()
            } else {
                Style::new()
                    .fg(Color::DarkGrey)
                    .bg(base_style.bg.unwrap_or(Color::Black))
            };
            for ch in item.source.chars().take(source_col_width as usize) {
                buffer.put_char(col, row, ch, &source_style);
                col += 1;
            }
            // Pad source column
            for _ in item.source.len().min(source_col_width as usize)..source_col_width as usize {
                buffer.put_char(col, row, ' ', base_style);
                col += 1;
            }

            // Right padding
            buffer.put_char(col, row, ' ', base_style);
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
