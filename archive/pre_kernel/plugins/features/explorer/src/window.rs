//! Explorer plugin window

use std::sync::Arc;

use reovim_core::{
    frame::FrameBuffer,
    highlight::Theme,
    plugin::{EditorContext, PanelPosition, PluginStateRegistry, PluginWindow, Rect, WindowConfig},
};

use crate::{
    file_colors::get_file_color,
    state::{ExplorerState, FileDetailsPopup, TreeStyle},
    tree_render::{TreeLineInfo, build_simple_prefix, build_tree_prefix},
};

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

            // Get flattened nodes with metadata for tree structure
            let flat_nodes = explorer.tree.flatten_with_metadata(explorer.show_hidden);

            // Apply filter if present
            let filtered_nodes: Vec<_> = if explorer.filter_text.is_empty() {
                flat_nodes.iter().collect()
            } else {
                let filter_lower = explorer.filter_text.to_lowercase();
                flat_nodes
                    .iter()
                    .filter(|fn_meta| fn_meta.node.name.to_lowercase().contains(&filter_lower))
                    .collect()
            };

            let height = bounds.height as usize;

            // Reserve last line for message/prompt if present
            let available_height = if explorer.message.is_some() {
                height.saturating_sub(1)
            } else {
                height
            };

            // Calculate visible range based on scroll offset
            let start = explorer.scroll_offset;
            let end = (start + available_height).min(filtered_nodes.len());

            // Render visible nodes with multi-segment coloring
            for (line_idx, (i, flat_node)) in filtered_nodes
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

                let node = flat_node.node;
                let is_cursor = i + start == explorer.cursor_index;
                let is_marked = explorer.selection.selected.contains(&node.path);

                // 1. Build tree prefix based on style setting
                let tree_prefix = match explorer.tree_style {
                    TreeStyle::None => "  ".repeat(node.depth),
                    TreeStyle::Simple => build_simple_prefix(node.depth),
                    TreeStyle::BoxDrawing => build_tree_prefix(
                        &TreeLineInfo {
                            vertical_lines: flat_node.vertical_lines.clone(),
                            is_last_child: flat_node.is_last_child,
                        },
                        node.depth,
                    ),
                };

                // 2. Get icon
                let icon = node.icon();

                // 3. Build marker
                let marker = if is_marked {
                    "* "
                } else if is_cursor {
                    "> "
                } else {
                    "  "
                };

                // 4. Build size display
                let size_display = if explorer.show_sizes && !node.is_dir() {
                    if let Some(size) = node.size() {
                        format!(" {:>5} ", super::node::format_size(size))
                    } else {
                        String::new()
                    }
                } else {
                    String::new()
                };

                // 5. Determine colors
                let file_color = if explorer.enable_colors {
                    get_file_color(node, theme)
                } else {
                    theme.base.default.clone()
                };

                // Selection style overrides file color for the whole line
                let line_style = if is_cursor || is_marked {
                    &theme.selection.visual
                } else {
                    &file_color
                };

                // 6. Render segments with appropriate colors
                let mut x = bounds.x;

                // Marker (cursor/selection indicator) - always use line style
                x += buffer.write_str(x, y, marker, line_style);

                // Tree structure (dimmed) - unless selected
                let tree_style = if is_cursor || is_marked {
                    line_style
                } else {
                    &theme.fold.marker
                };
                x += buffer.write_str(x, y, &tree_prefix, tree_style);

                // Icon - use line style
                x += buffer.write_str(x, y, icon, line_style);

                // Filename - use line style
                x += buffer.write_str(x, y, &node.name, line_style);

                // Size display (dimmed) - unless selected
                if !size_display.is_empty() {
                    let size_style = if is_cursor || is_marked {
                        line_style
                    } else {
                        &theme.virtual_text.default
                    };
                    x += buffer.write_str(x, y, &size_display, size_style);
                }

                // Fill rest of line with spaces
                for col in x..(bounds.x + bounds.width) {
                    buffer.put_char(col, y, ' ', line_style);
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

            // Show hidden items count at bottom when hidden files are not shown
            if !explorer.show_hidden && explorer.message.is_none() {
                let hidden_count = count_hidden_items(&flat_nodes);
                if hidden_count > 0 {
                    let y = bounds.y + available_height.saturating_sub(1) as u16;
                    if y < bounds.y + bounds.height {
                        let text = format!("({} hidden)", hidden_count);
                        let style = &theme.virtual_text.default;

                        // Center the text
                        let text_width = text.len() as u16;
                        let x_offset = if text_width < bounds.width {
                            (bounds.width - text_width) / 2
                        } else {
                            0
                        };
                        let x = bounds.x + x_offset;

                        buffer.write_str(x, y, &text, style);
                    }
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

/// Count hidden items in the tree
///
/// Counts all hidden items that exist but are not shown in the flattened view.
fn count_hidden_items<'a>(flat_nodes: &[crate::tree::FlattenedNode<'a>]) -> usize {
    // Count all children that are hidden (not in the flattened list)
    // This traverses all expanded directories and counts their hidden children
    let mut hidden_count = 0;

    for flat_node in flat_nodes {
        let node = flat_node.node;

        // If this node is expanded and a directory, check for hidden children
        if node.is_expanded()
            && node.is_dir()
            && let Some(children) = node.children()
        {
            for child in children {
                if child.is_hidden {
                    // Count this hidden item and all its descendants
                    hidden_count += 1 + count_hidden_recursive(child);
                }
            }
        }
    }

    hidden_count
}

/// Recursively count all descendants of a hidden node
fn count_hidden_recursive(node: &crate::node::FileNode) -> usize {
    let mut count = 0;

    if let Some(children) = node.children() {
        for child in children {
            count += 1 + count_hidden_recursive(child);
        }
    }

    count
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
