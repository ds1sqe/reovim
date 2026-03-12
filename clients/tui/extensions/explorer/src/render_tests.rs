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
fn render_narrow_sidebar_prefix_overflow() {
    use reovim_driver_display::FrameBuffer;

    // Deeply nested node: prefix "│ │ └─" = 6 chars + icon "  " = 2 chars.
    // With width=4, max_x = x + 3, so prefix loop hits `col >= max_x` break.
    let data = make_data(vec![NodeData {
        name: "deep.rs".to_owned(),
        depth: 3,
        is_dir: false,
        is_expanded: false,
        is_hidden: false,
        is_last: true,
        vertical_lines: vec![true, true, false],
        is_symlink: false,
        size: 0,
    }]);
    let bounds = crate::layout::SidebarBounds::calculate(4, 10, false);
    let mut fb = FrameBuffer::new(40, 10);
    render_explorer(&mut fb, &data, &bounds);
    // Should not panic — prefix truncated before icon is reached
    assert!(fb.get(0, 0).is_some());
}

#[test]
fn render_narrow_sidebar_icon_overflow() {
    use reovim_driver_display::FrameBuffer;

    // Depth 1 node: prefix "└─" = 2 chars, icon "> " = 2 chars.
    // With width=5 (margin 1 + 3 cols before max_x), prefix fits but icon hits break.
    let data = make_data(vec![NodeData {
        name: "src".to_owned(),
        depth: 1,
        is_dir: true,
        is_expanded: false,
        is_hidden: false,
        is_last: true,
        vertical_lines: vec![false],
        is_symlink: false,
        size: 0,
    }]);
    let bounds = crate::layout::SidebarBounds::calculate(5, 10, false);
    let mut fb = FrameBuffer::new(40, 10);
    render_explorer(&mut fb, &data, &bounds);
    // Should not panic — icon truncated
    assert!(fb.get(0, 0).is_some());
}

#[test]
fn build_prefix_depth_3() {
    let prefix = build_prefix(&[true, true, false], true, 3);
    // │ │ └─
    assert!(prefix.contains('\u{2502}'));
    assert!(prefix.contains('\u{2514}'));
}
