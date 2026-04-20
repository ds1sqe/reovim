use {super::*, reovim_client_driver::testing::WriteSurface};

// =============================================================================
// Helpers
// =============================================================================

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

// =============================================================================
// render_explorer tests
// =============================================================================

#[test]
fn render_explorer_basic() {
    let data = make_data(vec![make_node("src", 0, true)]);
    let bounds = crate::layout::SidebarBounds::calculate(0, 0, 30, 10, false);
    let mut surface = WriteSurface::new(40, 10);
    render_explorer(&mut surface, &data, &bounds);

    // Header row should contain "project"
    let has_header = surface
        .writes()
        .iter()
        .any(|w| w.x == 1 && w.y == 0 && w.text == "project");
    assert!(has_header, "Expected header 'project' at (1, 0)");
}

#[test]
fn render_with_cursor_on_node() {
    let mut data = make_data(vec![make_node("first", 0, false), make_node("second", 0, false)]);
    data.cursor_index = 1;
    let bounds = crate::layout::SidebarBounds::calculate(0, 0, 30, 10, false);
    let mut surface = WriteSurface::new(40, 10);
    render_explorer(&mut surface, &data, &bounds);

    // Both rows should render without panic
    assert!(!surface.writes().is_empty());
}

#[test]
fn render_with_input_prompt() {
    let mut data = make_data(vec![]);
    data.input_mode = "createFile".to_owned();
    data.input_label = "New file: ".to_owned();
    data.input_buffer = "test.rs".to_owned();
    let bounds = crate::layout::SidebarBounds::calculate(0, 0, 30, 10, true);
    let mut surface = WriteSurface::new(40, 10);
    render_explorer(&mut surface, &data, &bounds);

    // Input prompt should be rendered at the bottom
    let input_y = bounds.input_y.unwrap();
    let has_prompt = surface
        .writes()
        .iter()
        .any(|w| w.y == input_y && w.text.contains("New file:"));
    assert!(has_prompt, "Expected input prompt at bottom row");
}

#[test]
fn render_with_message() {
    let mut data = make_data(vec![]);
    data.message = Some("File created".to_owned());
    let bounds = crate::layout::SidebarBounds::calculate(0, 0, 30, 10, false);
    let mut surface = WriteSurface::new(40, 10);
    render_explorer(&mut surface, &data, &bounds);

    // Message should appear near bottom
    let msg_y = bounds.y + bounds.height.saturating_sub(1);
    let has_msg = surface
        .writes()
        .iter()
        .any(|w| w.y == msg_y && w.text.contains("File created"));
    assert!(has_msg, "Expected message near bottom");
}

#[test]
fn render_separator_column() {
    let data = make_data(vec![]);
    let bounds = crate::layout::SidebarBounds::calculate(0, 0, 30, 10, false);
    let mut surface = WriteSurface::new(40, 10);
    render_explorer(&mut surface, &data, &bounds);

    // Separator at x=29 (width-1)
    let ch = surface.text_at(29, 0);
    assert_eq!(ch, Some("\u{2502}")); // |
}

#[test]
fn render_expanded_dir_icon() {
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
    let bounds = crate::layout::SidebarBounds::calculate(0, 0, 30, 10, false);
    let mut surface = WriteSurface::new(40, 10);
    render_explorer(&mut surface, &data, &bounds);

    // Nerd Font icon for expanded "src" dir at row 1 (tree_start_y), col 1 (margin)
    let ch = surface.text_at(1, 1);
    assert!(ch.is_some(), "expanded dir should have an icon at (1,1)");
    assert_ne!(ch, Some(" "), "expanded dir icon should not be a space");
}

#[test]
fn render_collapsed_dir_icon() {
    let data = make_data(vec![make_node("lib", 0, true)]);
    let bounds = crate::layout::SidebarBounds::calculate(0, 0, 30, 10, false);
    let mut surface = WriteSurface::new(40, 10);
    render_explorer(&mut surface, &data, &bounds);

    // Nerd Font icon for collapsed dir
    let ch = surface.text_at(1, 1);
    assert!(ch.is_some(), "collapsed dir should have an icon at (1,1)");
    assert_ne!(ch, Some(" "), "collapsed dir icon should not be a space");
}

#[test]
fn render_symlink_icon() {
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
    let bounds = crate::layout::SidebarBounds::calculate(0, 0, 30, 10, false);
    let mut surface = WriteSurface::new(40, 10);
    render_explorer(&mut surface, &data, &bounds);

    // '@' icon preserved for symlinks
    let ch = surface.text_at(1, 1);
    assert_eq!(ch, Some("@"));
}

#[test]
fn render_at_offset() {
    let data = make_data(vec![make_node("src", 0, true)]);
    let bounds = crate::layout::SidebarBounds::calculate(5, 3, 30, 10, false);
    let mut surface = WriteSurface::new(80, 24);
    render_explorer(&mut surface, &data, &bounds);

    // Header should be at (6, 3) -- bounds.x + 1, bounds.header_y
    let has_header = surface
        .writes()
        .iter()
        .any(|w| w.x == 6 && w.y == 3 && w.text == "project");
    assert!(has_header, "Expected header at offset (6, 3)");
}

#[test]
fn render_scroll_offset() {
    let mut data = make_data(vec![
        make_node("a", 0, false),
        make_node("b", 0, false),
        make_node("c", 0, false),
    ]);
    data.scroll_offset = 1;
    let bounds = crate::layout::SidebarBounds::calculate(0, 0, 30, 10, false);
    let mut surface = WriteSurface::new(40, 10);
    render_explorer(&mut surface, &data, &bounds);

    // First visible node should be "b" (index 1)
    // At row 1 (tree_start_y), col 3 (margin + 2 spaces for file icon)
    let ch = surface.text_at(3, 1);
    assert_eq!(ch, Some("b"));
}

#[test]
fn render_truncated_header() {
    let mut data = make_data(vec![]);
    data.root_name = "a".repeat(100);
    data.width = 10;
    let bounds = crate::layout::SidebarBounds::calculate(0, 0, 10, 10, false);
    let mut surface = WriteSurface::new(40, 10);
    render_explorer(&mut surface, &data, &bounds);
    // Should not panic despite long name
    assert!(!surface.writes().is_empty());
}

#[test]
fn render_message_with_input_mode() {
    let mut data = make_data(vec![]);
    data.message = Some("Error".to_owned());
    data.input_mode = "createFile".to_owned();
    data.input_label = "New file: ".to_owned();
    let bounds = crate::layout::SidebarBounds::calculate(0, 0, 30, 10, true);
    let mut surface = WriteSurface::new(40, 10);
    render_explorer(&mut surface, &data, &bounds);

    // Message should render above input (y + height - 2)
    let msg_y = bounds.y + bounds.height.saturating_sub(2);
    let has_msg = surface
        .writes()
        .iter()
        .any(|w| w.y == msg_y && w.text.contains("Error"));
    assert!(has_msg, "Expected message above input row");
}

// =============================================================================
// build_prefix tests
// =============================================================================

#[test]
fn build_prefix_root() {
    assert_eq!(build_prefix(&[], true, 0), "");
}

#[test]
fn build_prefix_depth_1_not_last() {
    assert_eq!(build_prefix(&[true], false, 1), "\u{251c}\u{2500}"); // |-
}

#[test]
fn build_prefix_depth_1_last() {
    assert_eq!(build_prefix(&[false], true, 1), "\u{2514}\u{2500}"); // corner
}

#[test]
fn build_prefix_depth_2_continuation() {
    assert_eq!(
        build_prefix(&[true, false], true, 2),
        "\u{2502} \u{2514}\u{2500}" // | corner
    );
}

#[test]
fn build_prefix_depth_2_no_continuation() {
    assert_eq!(
        build_prefix(&[false, true], false, 2),
        "  \u{251c}\u{2500}" //   |-
    );
}

#[test]
fn build_prefix_depth_3() {
    let prefix = build_prefix(&[true, true, false], true, 3);
    assert!(prefix.contains('\u{2502}'));
    assert!(prefix.contains('\u{2514}'));
}

// ========================================================================
// File icon lookup tests
// ========================================================================

#[test]
fn file_icon_rust() {
    assert!(file_icon("main.rs").contains('\u{e7a8}'));
}

#[test]
fn file_icon_python() {
    assert!(file_icon("script.py").contains('\u{e73c}'));
}

#[test]
fn file_icon_unknown_extension() {
    let icon = file_icon("file.xyz");
    assert!(!icon.is_empty(), "unknown extension should get generic file icon");
}

#[test]
fn file_icon_no_extension() {
    let icon = file_icon("Makefile");
    assert!(!icon.is_empty(), "special names should get an icon");
}

#[test]
fn dir_icon_git() {
    let icon = dir_icon_collapsed(".git");
    assert!(icon.contains('\u{e702}'));
}

#[test]
fn dir_icon_generic_collapsed() {
    let icon = dir_icon_collapsed("my_dir");
    assert!(!icon.is_empty());
}

#[test]
fn dir_icon_generic_expanded() {
    let icon = dir_icon_expanded("my_dir");
    assert!(!icon.is_empty());
}

#[test]
fn dir_icon_collapsed_vs_expanded_differ() {
    let collapsed = dir_icon_collapsed("my_dir");
    let expanded = dir_icon_expanded("my_dir");
    assert_ne!(collapsed, expanded, "collapsed and expanded generic dirs should differ");
}
