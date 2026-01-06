//! Explorer plugin window

use std::sync::Arc;

use reovim_core::{
    frame::FrameBuffer,
    highlight::Theme,
    plugin::{EditorContext, PanelPosition, PluginStateRegistry, PluginWindow, Rect, WindowConfig},
};

use crate::state::{ExplorerState, FileDetailsPopup};

/// Unified plugin window for the explorer
///
/// Handles both window configuration and rendering through the `PluginWindow` trait.
pub struct ExplorerPluginWindow;

impl PluginWindow for ExplorerPluginWindow {
    fn window_config(
        &self,
        state: &Arc<PluginStateRegistry>,
        ctx: &EditorContext,
    ) -> Option<WindowConfig> {
        // Check if explorer is visible
        let is_visible = state
            .with::<ExplorerState, _, _>(|explorer| explorer.visible)
            .unwrap_or(false);

        if !is_visible {
            return None;
        }

        // Get explorer width from state
        let width = state
            .with::<ExplorerState, _, _>(|explorer| explorer.width)
            .unwrap_or(30);

        // Use EditorContext helper to calculate left sidebar position
        let (x, y, w, h) = ctx.side_panel(PanelPosition::Left, width);

        Some(WindowConfig {
            bounds: Rect::new(x, y, w, h),
            z_order: 150, // Between editor windows (100) and overlays (200+)
            visible: true,
        })
    }

    fn render(
        &self,
        state: &Arc<PluginStateRegistry>,
        _ctx: &EditorContext,
        buffer: &mut FrameBuffer,
        bounds: Rect,
        theme: &Theme,
    ) {
        // Update visible height for scroll calculations
        state.with_mut::<ExplorerState, _, _>(|explorer| {
            explorer.set_visible_height(bounds.height);
        });

        // Get explorer state and render
        state.with::<ExplorerState, _, _>(|explorer| {
            if !explorer.visible {
                return;
            }

            let nodes = explorer.visible_nodes();
            let height = bounds.height as usize;

            // Reserve last line for message/prompt if present
            let available_height = if explorer.message.is_some() {
                height.saturating_sub(1)
            } else {
                height
            };

            // Calculate visible range based on scroll offset
            let start = explorer.scroll_offset;
            let end = (start + available_height).min(nodes.len());

            // Render visible nodes
            for (line_idx, (i, node)) in nodes
                .iter()
                .enumerate()
                .skip(start)
                .take(end - start)
                .enumerate()
            {
                let y = bounds.y + line_idx as u16;
                if y >= bounds.y + bounds.height {
                    break;
                }

                let is_cursor = i == explorer.cursor_index;
                let is_marked = explorer.selection.selected.contains(&node.path);

                // Build line content
                let indent = "  ".repeat(node.depth);
                let icon = node.icon(); // Uses IconRegistry for file/dir icons

                let marker = if is_marked {
                    "* "
                } else if is_cursor {
                    "> "
                } else {
                    "  "
                };

                let size_display = if explorer.show_sizes && !node.is_dir() {
                    if let Some(size) = node.size() {
                        format!(" {:>5} ", super::node::format_size(size))
                    } else {
                        String::new()
                    }
                } else {
                    String::new()
                };

                let line = format!("{marker}{indent}{icon}{}{size_display}", node.name);

                // Determine style
                let style = if is_cursor || is_marked {
                    &theme.selection.visual
                } else {
                    &theme.base.default
                };

                // Render the line using write_str (handles wide characters properly)
                let x = bounds.x + buffer.write_str(bounds.x, y, &line, style);

                // Fill rest of line with spaces
                for col in x..(bounds.x + bounds.width) {
                    buffer.put_char(col, y, ' ', style);
                }
            }

            // Fill remaining lines with empty space
            let rendered_lines = end.saturating_sub(start);
            for line_idx in rendered_lines..available_height {
                let y = bounds.y + line_idx as u16;
                if y >= bounds.y + bounds.height {
                    break;
                }
                for x in bounds.x..(bounds.x + bounds.width) {
                    buffer.put_char(x, y, ' ', &theme.base.default);
                }
            }

            // Render message/input prompt at bottom if present
            if let Some(ref message) = explorer.message {
                let y = bounds.y + available_height as u16;
                if y < bounds.y + bounds.height {
                    let input_display = if !explorer.input_buffer.is_empty() {
                        explorer.input_buffer.as_str()
                    } else {
                        "_"
                    };
                    let prompt = format!("{message}{input_display}");

                    let mut x = bounds.x;
                    for ch in prompt.chars() {
                        if x >= bounds.x + bounds.width {
                            break;
                        }
                        buffer.put_char(x, y, ch, &theme.statusline.background);
                        x += 1;
                    }

                    // Fill rest with status line background
                    while x < bounds.x + bounds.width {
                        buffer.put_char(x, y, ' ', &theme.statusline.background);
                        x += 1;
                    }
                }
            }
        });
    }
}

/// File details popup window
///
/// Shows detailed information about the selected file/directory.
pub struct FileDetailsPluginWindow;

impl PluginWindow for FileDetailsPluginWindow {
    fn window_config(
        &self,
        state: &Arc<PluginStateRegistry>,
        ctx: &EditorContext,
    ) -> Option<WindowConfig> {
        // Get popup visibility and cursor position from explorer state
        let (is_visible, cursor_screen_y, explorer_width) = state
            .with::<ExplorerState, _, _>(|explorer| {
                // Calculate cursor's screen Y position
                let cursor_y = explorer.cursor_index.saturating_sub(explorer.scroll_offset) as u16;
                (explorer.popup.visible, cursor_y, explorer.width)
            })
            .unwrap_or((false, 0, 30));

        if !is_visible {
            return None;
        }

        // Popup dimensions (compact: 45 chars wide, 8 lines tall)
        let width = 45u16.min(ctx.screen_width.saturating_sub(4));
        let height = 8u16;

        // Position popup to the right of explorer, below cursor
        let x = explorer_width + 1;

        // Calculate Y position: below cursor line
        let cursor_absolute_y = ctx.tab_line_height + cursor_screen_y;
        let popup_y = cursor_absolute_y + 1; // One line below cursor

        // Ensure popup doesn't go off the bottom of the screen
        let max_y = ctx
            .screen_height
            .saturating_sub(height + ctx.status_line_height);
        let y = popup_y.min(max_y);

        // If popup would go off screen to the right, position it at the right edge
        let max_x = ctx.screen_width.saturating_sub(width);
        let x = x.min(max_x);

        Some(WindowConfig {
            bounds: Rect::new(x, y, width, height),
            z_order: 350, // Above explorer (150), below modal (400)
            visible: true,
        })
    }

    fn render(
        &self,
        state: &Arc<PluginStateRegistry>,
        _ctx: &EditorContext,
        buffer: &mut FrameBuffer,
        bounds: Rect,
        theme: &Theme,
    ) {
        // Get popup state
        let popup = state
            .with::<ExplorerState, _, _>(|explorer| explorer.popup.clone())
            .unwrap_or_default();

        if !popup.visible {
            return;
        }

        let border_style = &theme.popup.border;
        let content_style = &theme.popup.normal;

        // Render the popup
        render_file_details_popup(buffer, bounds, &popup, border_style, content_style);
    }
}

/// Render the file details popup
#[allow(clippy::cast_possible_truncation)]
fn render_file_details_popup(
    buffer: &mut FrameBuffer,
    bounds: Rect,
    popup: &FileDetailsPopup,
    border_style: &reovim_core::highlight::Style,
    content_style: &reovim_core::highlight::Style,
) {
    let x = bounds.x;
    let y = bounds.y;
    let w = bounds.width;
    let h = bounds.height;

    // Top border with title
    let title = " File Details ";
    buffer.put_char(x, y, '╭', border_style);
    buffer.put_char(x + 1, y, '─', border_style);
    for (i, ch) in title.chars().enumerate() {
        buffer.put_char(x + 2 + i as u16, y, ch, border_style);
    }
    let title_end = 2 + title.len() as u16;
    for col in (x + title_end)..(x + w - 1) {
        buffer.put_char(col, y, '─', border_style);
    }
    buffer.put_char(x + w - 1, y, '╮', border_style);

    // Content lines (6 lines: Name, Path, Type, Size, Created, Modified)
    let labels = [
        "    Name:",
        "    Path:",
        "    Type:",
        "    Size:",
        " Created:",
        "Modified:",
    ];
    let values = [
        popup.name.clone(),
        truncate_path(&popup.path, (w - 12) as usize),
        popup.file_type.clone(),
        popup.size.clone().unwrap_or_else(|| "-".to_string()),
        popup.created.clone().unwrap_or_else(|| "-".to_string()),
        popup.modified.clone().unwrap_or_else(|| "-".to_string()),
    ];

    for (line_idx, (label, value)) in labels.iter().zip(values.iter()).enumerate() {
        let row = y + 1 + line_idx as u16;
        if row >= y + h - 1 {
            break;
        }

        // Left border
        buffer.put_char(x, row, '│', border_style);

        // Label (right-aligned)
        let content = format!("{label} {value}");
        let mut col = x + 1;
        for ch in content.chars() {
            if col >= x + w - 1 {
                break;
            }
            buffer.put_char(col, row, ch, content_style);
            col += 1;
        }

        // Fill rest with spaces
        while col < x + w - 1 {
            buffer.put_char(col, row, ' ', content_style);
            col += 1;
        }

        // Right border
        buffer.put_char(x + w - 1, row, '│', border_style);
    }

    // Bottom border with hints
    let bottom_y = y + h - 1;
    let hint = " <T>: copy path / <Esc/Enter> Close ";
    buffer.put_char(x, bottom_y, '╰', border_style);
    buffer.put_char(x + 1, bottom_y, '─', border_style);
    for (i, ch) in hint.chars().enumerate() {
        let col = x + 2 + i as u16;
        if col >= x + w - 1 {
            break;
        }
        buffer.put_char(col, bottom_y, ch, border_style);
    }
    let hint_end = 2 + hint.len() as u16;
    for col in (x + hint_end)..(x + w - 1) {
        buffer.put_char(col, bottom_y, '─', border_style);
    }
    buffer.put_char(x + w - 1, bottom_y, '╯', border_style);
}

/// Truncate a path to fit within a given width
fn truncate_path(path: &str, max_len: usize) -> String {
    if path.len() <= max_len {
        path.to_string()
    } else {
        // Keep the end of the path, prefix with "..."
        let keep = max_len.saturating_sub(3);
        format!("...{}", &path[path.len() - keep..])
    }
}
