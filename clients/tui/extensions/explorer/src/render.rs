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
mod tests {
    use super::*;

    #[test]
    fn build_prefix_root() {
        assert_eq!(build_prefix(&[], true, 0), "");
    }

    #[test]
    fn build_prefix_depth_1_not_last() {
        assert_eq!(build_prefix(&[true], false, 1), "\u{251c}\u{2500}"); // ├─
    }

    #[test]
    fn build_prefix_depth_1_last() {
        assert_eq!(build_prefix(&[false], true, 1), "\u{2514}\u{2500}"); // └─
    }

    #[test]
    fn build_prefix_depth_2_continuation() {
        assert_eq!(
            build_prefix(&[true, false], true, 2),
            "\u{2502} \u{2514}\u{2500}" // │ └─
        );
    }

    #[test]
    fn build_prefix_depth_2_no_continuation() {
        assert_eq!(
            build_prefix(&[false, true], false, 2),
            "  \u{251c}\u{2500}" //   ├─
        );
    }

    fn make_data(nodes: Vec<NodeData>) -> ExplorerData {
        ExplorerData {
            active: true,
            root_name: "project".to_owned(),
            cursor_index: 0,
            scroll_offset: 0,
            width: 30,
            input_mode: "none".to_owned(),
            input_buffer: String::new(),
            input_label: String::new(),
            message: None,
            show_hidden: false,
            nodes,
        }
    }

    fn make_node(name: &str, depth: usize, is_dir: bool) -> NodeData {
        NodeData {
            name: name.to_owned(),
            depth,
            is_dir,
            is_expanded: false,
            is_hidden: false,
            is_last: true,
            vertical_lines: vec![],
            is_symlink: false,
            size: 0,
        }
    }

    #[test]
    fn render_explorer_small() {
        use reovim_driver_display::FrameBuffer;

        let data = make_data(vec![make_node("src", 0, true)]);
        let bounds = crate::layout::SidebarBounds::calculate(30, 10, false);
        let mut fb = FrameBuffer::new(40, 10);
        render_explorer(&mut fb, &data, &bounds);

        // Header row should contain "project"
        let header_char = fb.get(1, 0).map(|c| c.char);
        assert_eq!(header_char, Some('p'));
    }

    #[test]
    fn render_with_message() {
        use reovim_driver_display::FrameBuffer;

        let mut data = make_data(vec![]);
        data.message = Some("File created".to_owned());
        let bounds = crate::layout::SidebarBounds::calculate(30, 10, false);
        let mut fb = FrameBuffer::new(40, 10);
        render_explorer(&mut fb, &data, &bounds);

        // Message should appear near bottom
        let msg_y = bounds.height.saturating_sub(1);
        let ch = fb.get(1, msg_y).map(|c| c.char);
        assert_eq!(ch, Some('F'));
    }

    #[test]
    fn render_with_input_prompt() {
        use reovim_driver_display::FrameBuffer;

        let mut data = make_data(vec![]);
        data.input_mode = "createFile".to_owned();
        data.input_label = "New file: ".to_owned();
        data.input_buffer = "test.rs".to_owned();
        let bounds = crate::layout::SidebarBounds::calculate(30, 10, true);
        let mut fb = FrameBuffer::new(40, 10);
        render_explorer(&mut fb, &data, &bounds);

        // Input row should appear at the bottom
        let input_y = bounds.input_y.unwrap();
        let ch = fb.get(1, input_y).map(|c| c.char);
        assert_eq!(ch, Some('N'));
    }

    #[test]
    fn render_expanded_dir_node() {
        use reovim_driver_display::FrameBuffer;

        let data = make_data(vec![NodeData {
            name: "src".to_owned(),
            depth: 0,
            is_dir: true,
            is_expanded: true,
            is_hidden: false,
            is_last: true,
            vertical_lines: vec![],
            is_symlink: false,
            size: 0,
        }]);
        let bounds = crate::layout::SidebarBounds::calculate(30, 10, false);
        let mut fb = FrameBuffer::new(40, 10);
        render_explorer(&mut fb, &data, &bounds);
        // 'v' icon for expanded dir at row 1 (tree_start_y)
        let ch = fb.get(1, 1).map(|c| c.char);
        assert_eq!(ch, Some('v'));
    }

    #[test]
    fn render_collapsed_dir_node() {
        use reovim_driver_display::FrameBuffer;

        let data = make_data(vec![make_node("lib", 0, true)]);
        let bounds = crate::layout::SidebarBounds::calculate(30, 10, false);
        let mut fb = FrameBuffer::new(40, 10);
        render_explorer(&mut fb, &data, &bounds);
        // '>' icon for collapsed dir
        let ch = fb.get(1, 1).map(|c| c.char);
        assert_eq!(ch, Some('>'));
    }

    #[test]
    fn render_symlink_node() {
        use reovim_driver_display::FrameBuffer;

        let data = make_data(vec![NodeData {
            name: "link".to_owned(),
            depth: 0,
            is_dir: false,
            is_expanded: false,
            is_hidden: false,
            is_last: true,
            vertical_lines: vec![],
            is_symlink: true,
            size: 0,
        }]);
        let bounds = crate::layout::SidebarBounds::calculate(30, 10, false);
        let mut fb = FrameBuffer::new(40, 10);
        render_explorer(&mut fb, &data, &bounds);
        // '@' icon for symlink
        let ch = fb.get(1, 1).map(|c| c.char);
        assert_eq!(ch, Some('@'));
    }

    #[test]
    fn render_hidden_file_node() {
        use reovim_driver_display::FrameBuffer;

        let data = make_data(vec![NodeData {
            name: ".gitignore".to_owned(),
            depth: 0,
            is_dir: false,
            is_expanded: false,
            is_hidden: true,
            is_last: true,
            vertical_lines: vec![],
            is_symlink: false,
            size: 0,
        }]);
        let bounds = crate::layout::SidebarBounds::calculate(30, 10, false);
        let mut fb = FrameBuffer::new(40, 10);
        render_explorer(&mut fb, &data, &bounds);
        // Hidden file should still render
        let ch = fb.get(3, 1).map(|c| c.char);
        assert_eq!(ch, Some('.'));
    }

    #[test]
    fn render_tree_node_with_depth() {
        use reovim_driver_display::FrameBuffer;

        let data = make_data(vec![
            NodeData {
                name: "src".to_owned(),
                depth: 0,
                is_dir: true,
                is_expanded: true,
                is_hidden: false,
                is_last: false,
                vertical_lines: vec![],
                is_symlink: false,
                size: 0,
            },
            NodeData {
                name: "main.rs".to_owned(),
                depth: 1,
                is_dir: false,
                is_expanded: false,
                is_hidden: false,
                is_last: true,
                vertical_lines: vec![true],
                is_symlink: false,
                size: 100,
            },
        ]);
        let bounds = crate::layout::SidebarBounds::calculate(30, 10, false);
        let mut fb = FrameBuffer::new(40, 10);
        render_explorer(&mut fb, &data, &bounds);

        // First node at row 1, second at row 2
        let ch1 = fb.get(1, 1).map(|c| c.char);
        assert_eq!(ch1, Some('v')); // expanded dir icon

        // Second node should have prefix chars
        // The prefix for depth 1, is_last=true is "└─"
        let ch2 = fb.get(1, 2).map(|c| c.char);
        assert_eq!(ch2, Some('\u{2514}')); // └
    }

    #[test]
    fn render_cursor_row_different_bg() {
        use reovim_driver_display::FrameBuffer;

        let mut data = make_data(vec![make_node("first", 0, false), make_node("second", 0, false)]);
        data.cursor_index = 1;
        let bounds = crate::layout::SidebarBounds::calculate(30, 10, false);
        let mut fb = FrameBuffer::new(40, 10);
        render_explorer(&mut fb, &data, &bounds);

        // Both rows should render without panic
        assert!(fb.get(1, 1).is_some());
        assert!(fb.get(1, 2).is_some());
    }

    #[test]
    fn render_separator_column() {
        use reovim_driver_display::FrameBuffer;

        let data = make_data(vec![]);
        let bounds = crate::layout::SidebarBounds::calculate(30, 10, false);
        let mut fb = FrameBuffer::new(40, 10);
        render_explorer(&mut fb, &data, &bounds);

        // Separator at x=29 (width-1)
        let ch = fb.get(29, 0).map(|c| c.char);
        assert_eq!(ch, Some('\u{2502}')); // │
    }

    #[test]
    fn render_truncated_header() {
        use reovim_driver_display::FrameBuffer;

        let mut data = make_data(vec![]);
        data.root_name = "a".repeat(100);
        data.width = 10;
        let bounds = crate::layout::SidebarBounds::calculate(10, 10, false);
        let mut fb = FrameBuffer::new(40, 10);
        render_explorer(&mut fb, &data, &bounds);

        // Should not panic despite long name
        assert!(fb.get(1, 0).is_some());
    }

    #[test]
    fn render_message_with_input_mode() {
        use reovim_driver_display::FrameBuffer;

        let mut data = make_data(vec![]);
        data.message = Some("Error".to_owned());
        data.input_mode = "createFile".to_owned();
        data.input_label = "New file: ".to_owned();
        let bounds = crate::layout::SidebarBounds::calculate(30, 10, true);
        let mut fb = FrameBuffer::new(40, 10);
        render_explorer(&mut fb, &data, &bounds);

        // Message should render above input
        let msg_y = bounds.height.saturating_sub(2);
        let ch = fb.get(1, msg_y).map(|c| c.char);
        assert_eq!(ch, Some('E'));
    }

    #[test]
    fn render_scroll_offset() {
        use reovim_driver_display::FrameBuffer;

        let mut data = make_data(vec![
            make_node("a", 0, false),
            make_node("b", 0, false),
            make_node("c", 0, false),
        ]);
        data.scroll_offset = 1;
        let bounds = crate::layout::SidebarBounds::calculate(30, 10, false);
        let mut fb = FrameBuffer::new(40, 10);
        render_explorer(&mut fb, &data, &bounds);

        // First visible node should be "b" (index 1)
        let ch = fb.get(3, 1).map(|c| c.char);
        assert_eq!(ch, Some('b'));
    }

    #[test]
    fn render_narrow_sidebar() {
        use reovim_driver_display::FrameBuffer;

        let mut data = make_data(vec![make_node("very_long_filename.rs", 0, false)]);
        data.width = 5;
        let bounds = crate::layout::SidebarBounds::calculate(5, 10, false);
        let mut fb = FrameBuffer::new(40, 10);
        render_explorer(&mut fb, &data, &bounds);
        // Should not panic with very narrow sidebar
        assert!(fb.get(0, 0).is_some());
    }

    #[test]
    fn build_prefix_depth_3() {
        let prefix = build_prefix(&[true, true, false], true, 3);
        // │ │ └─
        assert!(prefix.contains('\u{2502}'));
        assert!(prefix.contains('\u{2514}'));
    }
}
