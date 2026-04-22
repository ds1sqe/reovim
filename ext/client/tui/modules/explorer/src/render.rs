//! Tree rendering for the explorer sidebar.

use reovim_client_driver::{ChromeSurface, Style, types::Color};

use crate::{ExplorerData, NodeData, layout::SidebarBounds};

/// Dark background color for the sidebar.
const SIDEBAR_BG: Color = Color::AnsiValue(235);

/// Slightly lighter background for the cursor row.
const CURSOR_BG: Color = Color::AnsiValue(238);

/// Input prompt background.
const INPUT_BG: Color = Color::AnsiValue(236);

/// Render the explorer sidebar into the given surface.
pub fn render_explorer(
    surface: &mut dyn ChromeSurface,
    data: &ExplorerData,
    bounds: &SidebarBounds,
) {
    render_background(surface, bounds);
    render_header(surface, data, bounds);
    render_tree_nodes(surface, data, bounds);
    render_separator(surface, bounds);
    if let Some(input_y) = bounds.input_y {
        render_input_prompt(surface, data, input_y, bounds.x, bounds.width);
    }
    if let Some(ref msg) = data.message {
        render_message(surface, msg, bounds);
    }
}

/// Fill sidebar area with background color.
fn render_background(surface: &mut dyn ChromeSurface, bounds: &SidebarBounds) {
    let style = Style::new().bg(SIDEBAR_BG);
    for row in 0..bounds.height {
        for col in 0..bounds.width.saturating_sub(1) {
            surface.write_styled(bounds.x + col, bounds.y + row, " ", style.clone());
        }
    }
}

/// Render the header (root directory name).
fn render_header(surface: &mut dyn ChromeSurface, data: &ExplorerData, bounds: &SidebarBounds) {
    let style = Style::new().fg(Color::White).bg(SIDEBAR_BG).bold();

    let available = bounds.width.saturating_sub(2) as usize;
    let display: String = if data.root_name.len() > available {
        data.root_name.chars().take(available).collect()
    } else {
        data.root_name.clone()
    };

    surface.write_styled(bounds.x + 1, bounds.header_y, &display, style);
}

/// Render the visible tree nodes.
#[allow(clippy::cast_possible_truncation)]
fn render_tree_nodes(surface: &mut dyn ChromeSurface, data: &ExplorerData, bounds: &SidebarBounds) {
    for row in 0..bounds.tree_height {
        let node_idx = data.scroll_offset + row as usize;
        if node_idx >= data.nodes.len() {
            break;
        }

        let node = &data.nodes[node_idx];
        let screen_y = bounds.tree_start_y + row;
        let is_cursor = node_idx == data.cursor_index;

        render_tree_node(surface, node, bounds.x, screen_y, bounds.width, is_cursor);
    }
}

/// Render a single tree node row.
#[cfg_attr(coverage_nightly, coverage(off))]
fn render_tree_node(
    surface: &mut dyn ChromeSurface,
    node: &NodeData,
    x: u16,
    y: u16,
    width: u16,
    is_cursor: bool,
) {
    let row_bg = if is_cursor { CURSOR_BG } else { SIDEBAR_BG };

    // Build the prefix from vertical_lines and is_last
    let prefix = build_prefix(&node.vertical_lines, node.is_last, node.depth);

    // Choose icon (Nerd Font for files/dirs, @ for symlinks)
    let icon = if node.is_dir {
        if node.is_expanded {
            dir_icon_expanded(&node.name)
        } else {
            dir_icon_collapsed(&node.name)
        }
    } else if node.is_symlink {
        "@ "
    } else {
        file_icon(&node.name)
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
        surface.write_styled(col, y, &ch.to_string(), prefix_style.clone());
        col += 1;
    }

    // Write icon
    for ch in icon.chars() {
        if col >= max_x {
            break;
        }
        surface.write_styled(col, y, &ch.to_string(), name_style.clone());
        col += 1;
    }

    // Write name
    for ch in node.name.chars() {
        if col >= max_x {
            break;
        }
        surface.write_styled(col, y, &ch.to_string(), name_style.clone());
        col += 1;
    }

    // Fill rest of row with background
    let fill_style = Style::new().bg(row_bg);
    while col < max_x {
        surface.write_styled(col, y, " ", fill_style.clone());
        col += 1;
    }
}

/// Build a tree prefix string from vertical lines and `is_last` info.
pub fn build_prefix(vertical_lines: &[bool], is_last: bool, depth: usize) -> String {
    if depth == 0 {
        return String::new();
    }

    let mut result = String::new();

    // For each ancestor depth level, draw continuation or blank
    for &has_continuation in vertical_lines.iter().take(depth.saturating_sub(1)) {
        if has_continuation {
            result.push_str("\u{2502} "); // | + space
        } else {
            result.push_str("  ");
        }
    }

    // Final connector for this node
    if is_last {
        result.push_str("\u{2514}\u{2500}"); // corner
    } else {
        result.push_str("\u{251c}\u{2500}"); // tee
    }

    result
}

/// Render the right-edge separator line.
fn render_separator(surface: &mut dyn ChromeSurface, bounds: &SidebarBounds) {
    let style = Style::new().fg(Color::DarkGrey).bg(SIDEBAR_BG);
    let sep_x = bounds.x + bounds.width - 1;
    for row in 0..bounds.height {
        surface.write_styled(sep_x, bounds.y + row, "\u{2502}", style.clone());
    }
}

/// Render the input prompt at the bottom of the sidebar.
fn render_input_prompt(
    surface: &mut dyn ChromeSurface,
    data: &ExplorerData,
    y: u16,
    x: u16,
    width: u16,
) {
    let style = Style::new().fg(Color::Yellow).bg(INPUT_BG);

    // Fill the row
    for col in 0..width.saturating_sub(1) {
        surface.write_styled(x + col, y, " ", style.clone());
    }

    // Write the prompt label + input buffer
    let prompt = format!("{}{}", data.input_label, data.input_buffer);
    surface.write_styled(x + 1, y, &prompt, style);
}

/// Render a status message near the bottom.
fn render_message(surface: &mut dyn ChromeSurface, msg: &str, bounds: &SidebarBounds) {
    let y = if bounds.input_y.is_some() {
        bounds.y + bounds.height.saturating_sub(2)
    } else {
        bounds.y + bounds.height.saturating_sub(1)
    };

    let style = Style::new().fg(Color::Yellow).bg(INPUT_BG);

    let available = bounds.width.saturating_sub(2) as usize;
    let display: String = msg.chars().take(available).collect();
    surface.write_styled(bounds.x + 1, y, &display, style);
}

// =============================================================================
// File type icons (Nerd Font)
// =============================================================================

/// Get Nerd Font icon for a file based on its name/extension.
fn file_icon(filename: &str) -> &'static str {
    let ext = filename.rsplit_once('.').map(|(_, e)| e);
    match ext {
        Some("rs") => "\u{e7a8} ",                         //
        Some("py" | "pyi") => "\u{e73c} ",                 //
        Some("js" | "mjs" | "cjs") => "\u{e781} ",         //
        Some("ts" | "mts" | "cts") => "\u{e628} ",         //
        Some("go") => "\u{e626} ",                         //
        Some("c" | "h") => "\u{e61e} ",                    //
        Some("cpp" | "hpp" | "cc" | "cxx") => "\u{e61d} ", //
        Some("sh" | "bash" | "zsh") => "\u{e795} ",        //
        Some("html" | "htm") => "\u{e736} ",               //
        Some("css" | "scss" | "sass") => "\u{e749} ",      //
        Some("md" | "markdown") => "\u{e73e} ",            //
        Some("json") => "\u{e60b} ",                       //
        Some("toml" | "yaml" | "yml") => "\u{e615} ",      //
        Some("lua") => "\u{e620} ",                        //
        Some("java") => "\u{e738} ",                       //
        Some("rb") => "\u{e739} ",                         //
        Some("lock") => "\u{f023} ",                       //
        Some("txt") => "\u{f0219} ",                       // 󰈙
        Some("xml") => "\u{e619} ",                        //
        Some("svg") => "\u{e60e} ",                        //
        _ => match filename {
            "Dockerfile" | "dockerfile" => "\u{e7b0} ",     //
            "Makefile" | "makefile" => "\u{e673} ",         //
            ".gitignore" | ".gitattributes" => "\u{e702} ", //
            ".env" | ".env.local" => "\u{f462} ",           //
            _ => "\u{f0219} ",                              // 󰈙 generic file
        },
    }
}

/// Get Nerd Font icon for a collapsed directory.
fn dir_icon_collapsed(dirname: &str) -> &'static str {
    match dirname {
        ".git" => "\u{e702} ",                      //
        "node_modules" => "\u{e71e} ",              //
        "target" | "build" | "dist" => "\u{f487} ", //  (gear)
        "src" | "lib" => "\u{f07c} ",               //  (folder open alt)
        "tests" | "test" | "spec" => "\u{f0668} ",  // 󰙨 (flask)
        "docs" | "doc" => "\u{f02d} ",              //  (book)
        _ => "\u{f024b} ",                          // 󰉋 closed folder
    }
}

/// Get Nerd Font icon for an expanded directory.
fn dir_icon_expanded(dirname: &str) -> &'static str {
    match dirname {
        ".git" => "\u{e702} ",                      //
        "node_modules" => "\u{e71e} ",              //
        "target" | "build" | "dist" => "\u{f487} ", //  (gear)
        "src" | "lib" => "\u{f07c} ",               //  (folder open alt)
        "tests" | "test" | "spec" => "\u{f0668} ",  // 󰙨 (flask)
        "docs" | "doc" => "\u{f02d} ",              //  (book)
        _ => "\u{f0770} ",                          // 󰝰 open folder
    }
}

#[cfg(test)]
#[path = "render_tests.rs"]
mod tests;
