//! Tree rendering for the explorer sidebar.

use {
    reovim_arch::Color,
    reovim_driver_display::{Style, render_backend::RenderBackend},
};

use crate::{ExplorerData, NodeData, layout::SidebarBounds};

/// Dark background color for the sidebar.
const SIDEBAR_BG: Color = Color::AnsiValue(235);

/// Slightly lighter background for the cursor row.
const CURSOR_BG: Color = Color::AnsiValue(238);

/// Input prompt background.
const INPUT_BG: Color = Color::AnsiValue(236);

/// Render the explorer sidebar.
pub(crate) fn render_explorer(
    backend: &mut dyn RenderBackend,
    data: &ExplorerData,
    bounds: &SidebarBounds,
) {
    render_background(backend, bounds);
    render_header(backend, data, bounds);
    render_tree_nodes(backend, data, bounds);
    render_separator(backend, bounds);
    if let Some(input_y) = bounds.input_y {
        render_input_prompt(backend, data, input_y, bounds.width);
    }
    if let Some(ref msg) = data.message {
        render_message(backend, msg, bounds);
    }
}

/// Fill sidebar area with background color.
fn render_background(backend: &mut dyn RenderBackend, bounds: &SidebarBounds) {
    let style = Style::new().bg(SIDEBAR_BG);
    for row in 0..bounds.height {
        for col in 0..bounds.width.saturating_sub(1) {
            backend.set_cell(bounds.x + col, bounds.y + row, ' ', &style);
        }
    }
}

/// Render the header (root directory name).
fn render_header(backend: &mut dyn RenderBackend, data: &ExplorerData, bounds: &SidebarBounds) {
    let style = Style::new().fg(Color::White).bg(SIDEBAR_BG).bold();

    let available = bounds.width.saturating_sub(2) as usize;
    let display: String = if data.root_name.len() > available {
        data.root_name.chars().take(available).collect()
    } else {
        data.root_name.clone()
    };

    backend.write_str(bounds.x + 1, bounds.header_y, &display, &style);
}

/// Render the visible tree nodes.
#[allow(clippy::cast_possible_truncation)]
fn render_tree_nodes(backend: &mut dyn RenderBackend, data: &ExplorerData, bounds: &SidebarBounds) {
    for row in 0..bounds.tree_height {
        let node_idx = data.scroll_offset + row as usize;
        if node_idx >= data.nodes.len() {
            break;
        }

        let node = &data.nodes[node_idx];
        let screen_y = bounds.tree_start_y + row;
        let is_cursor = node_idx == data.cursor_index;

        render_tree_node(backend, node, bounds.x, screen_y, bounds.width, is_cursor);
    }
}

/// Render a single tree node row.
fn render_tree_node(
    backend: &mut dyn RenderBackend,
    node: &NodeData,
    x: u16,
    y: u16,
    width: u16,
    is_cursor: bool,
) {
    let row_bg = if is_cursor { CURSOR_BG } else { SIDEBAR_BG };

    // Build the prefix from vertical_lines and is_last
    let prefix = build_prefix(&node.vertical_lines, node.is_last, node.depth);

    // Choose icon
    let icon = if node.is_dir {
        if node.is_expanded { "v " } else { "> " }
    } else if node.is_symlink {
        "@ "
    } else {
        "  "
    };

    // Choose name color
    let name_color = if node.is_hidden {
        Color::DarkGrey
    } else if node.is_dir {
        Color::Blue
    } else if node.is_symlink {
        Color::Cyan
    } else {
        Color::White
    };

    let mut name_style = Style::new().fg(name_color).bg(row_bg);
    if node.is_dir {
        name_style = name_style.bold();
    }
    let prefix_style = Style::new().fg(Color::DarkGrey).bg(row_bg);

    // Write prefix
    let mut col = x + 1; // 1 col left margin
    let max_x = x + width.saturating_sub(1);

    for ch in prefix.chars() {
        if col >= max_x {
            break;
        }
        backend.set_cell(col, y, ch, &prefix_style);
        col += 1;
    }

    // Write icon
    for ch in icon.chars() {
        if col >= max_x {
            break;
        }
        backend.set_cell(col, y, ch, &name_style);
        col += 1;
    }

    // Write name
    for ch in node.name.chars() {
        if col >= max_x {
            break;
        }
        backend.set_cell(col, y, ch, &name_style);
        col += 1;
    }

    // Fill rest of row with background
    let fill_style = Style::new().bg(row_bg);
    while col < max_x {
        backend.set_cell(col, y, ' ', &fill_style);
        col += 1;
    }
}

/// Build a tree prefix string from vertical lines and `is_last` info.
fn build_prefix(vertical_lines: &[bool], is_last: bool, depth: usize) -> String {
    if depth == 0 {
        return String::new();
    }

    let mut result = String::new();

    // For each ancestor depth level, draw continuation or blank
    for &has_continuation in vertical_lines.iter().take(depth.saturating_sub(1)) {
        if has_continuation {
            result.push_str("\u{2502} "); // │ + space
        } else {
            result.push_str("  ");
        }
    }

    // Final connector for this node
    if is_last {
        result.push_str("\u{2514}\u{2500}"); // └─
    } else {
        result.push_str("\u{251c}\u{2500}"); // ├─
    }

    result
}

/// Render the right-edge separator line.
fn render_separator(backend: &mut dyn RenderBackend, bounds: &SidebarBounds) {
    let style = Style::new().fg(Color::DarkGrey).bg(SIDEBAR_BG);
    let sep_x = bounds.x + bounds.width - 1;
    for row in 0..bounds.height {
        backend.set_cell(sep_x, bounds.y + row, '\u{2502}', &style); // │
    }
}

/// Render the input prompt at the bottom of the sidebar.
fn render_input_prompt(backend: &mut dyn RenderBackend, data: &ExplorerData, y: u16, width: u16) {
    let style = Style::new().fg(Color::Yellow).bg(INPUT_BG);

    // Fill the row
    for col in 0..width.saturating_sub(1) {
        backend.set_cell(col, y, ' ', &style);
    }

    // Write the prompt label + input buffer
    let prompt = format!("{}{}", data.input_label, data.input_buffer);
    backend.write_str(1, y, &prompt, &style);
}

/// Render a status message near the bottom.
fn render_message(backend: &mut dyn RenderBackend, msg: &str, bounds: &SidebarBounds) {
    let y = if bounds.input_y.is_some() {
        bounds.height.saturating_sub(2)
    } else {
        bounds.height.saturating_sub(1)
    };

    let style = Style::new().fg(Color::Yellow).bg(INPUT_BG);

    let available = bounds.width.saturating_sub(2) as usize;
    let display: String = msg.chars().take(available).collect();
    backend.write_str(bounds.x + 1, y, &display, &style);
}

#[cfg(test)]
#[path = "render_tests.rs"]
mod tests;
