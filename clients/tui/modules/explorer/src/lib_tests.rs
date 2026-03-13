use {
    super::*,
    reovim_client_driver::{Style, types::Color},
};

// =============================================================================
// MockSurface
// =============================================================================

struct MockSurface {
    writes: Vec<(u16, u16, String, Style)>,
    width: u16,
    height: u16,
}

impl MockSurface {
    fn new(width: u16, height: u16) -> Self {
        Self {
            writes: Vec::new(),
            width,
            height,
        }
    }
}

impl RenderSurface for MockSurface {
    fn write_styled(&mut self, x: u16, y: u16, text: &str, style: Style) -> u16 {
        #[allow(clippy::cast_possible_truncation)]
        let len = text.len() as u16;
        self.writes.push((x, y, text.to_string(), style));
        len
    }

    fn apply_style(&mut self, _x: u16, _y: u16, _style: Style) {}
    fn overlay_bg(&mut self, _x: u16, _y: u16, _bg: Color) {}
    fn fill(&mut self, _rect: Rect, _ch: char, _style: Style) {}
    fn clear(&mut self, _rect: Rect) {}
    fn size(&self) -> (u16, u16) {
        (self.width, self.height)
    }
}

// =============================================================================
// TestPlatformCaps
// =============================================================================

struct TestPlatformCaps;

impl PlatformCapabilities for TestPlatformCaps {
    fn rendering_model(&self) -> reovim_client_driver::RenderingModel {
        reovim_client_driver::RenderingModel::CellGrid
    }
    fn grid_size(&self) -> Option<(u16, u16)> {
        Some((80, 24))
    }
    fn color_depth(&self) -> reovim_client_driver::types::ColorDepth {
        reovim_client_driver::types::ColorDepth::TrueColor
    }
    fn pixel_size(&self) -> Option<(u32, u32)> {
        None
    }
    fn reliable_unicode_width(&self) -> bool {
        true
    }
    fn dark_mode(&self) -> bool {
        true
    }
    fn smooth_scroll(&self) -> bool {
        false
    }
    fn pointer_events(&self) -> bool {
        true
    }
    fn touch_input(&self) -> bool {
        false
    }
    fn haptic(&self) -> bool {
        false
    }
    fn safe_area(&self) -> reovim_client_driver::Insets {
        reovim_client_driver::Insets::ZERO
    }
    fn has_focus(&self) -> bool {
        true
    }
    fn clipboard_available(&self) -> bool {
        true
    }
    fn screen_reader_active(&self) -> bool {
        false
    }
}

fn test_caps() -> TestPlatformCaps {
    TestPlatformCaps
}

// =============================================================================
// Helper
// =============================================================================

fn active_payload(root: &str, nodes: &[(&str, usize, bool)]) -> String {
    let nodes_json: String = nodes
        .iter()
        .map(|(name, depth, is_dir)| {
            format!(
                r#"{{"name":"{name}","depth":{depth},"isDir":{is_dir},"isExpanded":false,"isHidden":false,"isLast":false,"verticalLines":[],"isSymlink":false,"size":0}}"#
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    format!(
        r#"{{"active":true,"rootName":"{root}","cursorIndex":0,"scrollOffset":0,"width":30,"inputMode":"none","inputBuffer":"","showHidden":false,"nodes":[{nodes_json}]}}"#
    )
}

// =============================================================================
// Identity tests
// =============================================================================

#[test]
fn explorer_id() {
    let m = ExplorerModule::new();
    assert_eq!(m.id(), "explorer");
}

#[test]
fn explorer_kind() {
    let m = ExplorerModule::new();
    assert_eq!(m.kind(), "explorer");
}

#[test]
fn explorer_name() {
    let m = ExplorerModule::new();
    assert_eq!(m.name(), "Explorer");
}

#[test]
fn explorer_version() {
    let m = ExplorerModule::new();
    assert_eq!(m.version(), Version::new(0, 1, 0));
}

// =============================================================================
// Default state tests
// =============================================================================

#[test]
fn default_is_inactive() {
    let m = ExplorerModule::new();
    assert!(!m.data.active);
}

#[test]
fn default_impl() {
    let m = ExplorerModule::default();
    assert!(!m.data.active);
}

// =============================================================================
// Chrome role tests
// =============================================================================

#[test]
fn has_chrome() {
    let m = ExplorerModule::new();
    assert!(m.has_chrome());
}

#[test]
fn chrome_position_left() {
    let m = ExplorerModule::new();
    assert_eq!(m.chrome_position(), ChromePosition::Left);
}

#[test]
fn chrome_priority_60() {
    let m = ExplorerModule::new();
    assert_eq!(m.chrome_priority(), 60);
}

#[test]
fn chrome_requested_size_inactive() {
    let m = ExplorerModule::new();
    assert_eq!(m.chrome_requested_size(&test_caps()), 0);
}

#[test]
fn chrome_requested_size_active() {
    let mut m = ExplorerModule::new();
    m.on_notification(&active_payload("proj", &[]));
    assert_eq!(m.chrome_requested_size(&test_caps()), 30);
}

// =============================================================================
// Lifecycle tests
// =============================================================================

#[test]
fn exit_ok() {
    let mut m = ExplorerModule::new();
    assert!(m.exit().is_ok());
}

// =============================================================================
// Notification tests
// =============================================================================

#[test]
fn notification_activate() {
    let mut m = ExplorerModule::new();
    m.on_notification(&active_payload("my-project", &[]));
    assert!(m.data.active);
    assert_eq!(m.data.root_name, "my-project");
}

#[test]
fn notification_deactivate() {
    let mut m = ExplorerModule::new();
    m.on_notification(&active_payload("proj", &[]));
    assert!(m.data.active);
    m.on_notification(r#"{"active":false}"#);
    assert!(!m.data.active);
}

#[test]
fn notification_invalid_json() {
    let mut m = ExplorerModule::new();
    m.on_notification("not json");
    assert!(!m.data.active);
}

#[test]
fn notification_nodes_parsing() {
    let mut m = ExplorerModule::new();
    m.on_notification(&active_payload("proj", &[("src", 0, true), ("main.rs", 1, false)]));
    assert_eq!(m.data.nodes.len(), 2);
    assert_eq!(m.data.nodes[0].name, "src");
    assert!(m.data.nodes[0].is_dir);
    assert_eq!(m.data.nodes[1].name, "main.rs");
    assert!(!m.data.nodes[1].is_dir);
}

#[test]
fn notification_delta_snapshot_preserves_nodes() {
    let mut m = ExplorerModule::new();
    // Full notification with nodes
    m.on_notification(&active_payload("proj", &[("src", 0, true)]));
    assert_eq!(m.data.nodes.len(), 1);
    assert_eq!(m.data.nodes[0].name, "src");

    // Delta notification (no "nodes" key) - nodes preserved
    m.on_notification(
        r#"{"active":true,"rootName":"proj","cursorIndex":5,"scrollOffset":2,"width":30,"inputMode":"none","inputBuffer":"","showHidden":false}"#,
    );
    assert_eq!(m.data.nodes.len(), 1);
    assert_eq!(m.data.nodes[0].name, "src");
    assert_eq!(m.data.cursor_index, 5);
    assert_eq!(m.data.scroll_offset, 2);
}

#[test]
fn notification_full_replaces_nodes() {
    let mut m = ExplorerModule::new();
    m.on_notification(&active_payload("proj", &[("old", 0, false)]));
    assert_eq!(m.data.nodes[0].name, "old");

    m.on_notification(&active_payload("proj", &[("new1", 0, true), ("new2", 1, false)]));
    assert_eq!(m.data.nodes.len(), 2);
    assert_eq!(m.data.nodes[0].name, "new1");
    assert_eq!(m.data.nodes[1].name, "new2");
}

#[test]
fn notification_input_mode_labels() {
    let mut m = ExplorerModule::new();
    m.on_notification(
        r#"{"active":true,"rootName":"x","cursorIndex":0,"scrollOffset":0,"width":30,"inputMode":"createFile","inputBuffer":"test.rs","showHidden":false,"nodes":[]}"#,
    );
    assert_eq!(m.data.input_mode, "createFile");
    assert_eq!(m.data.input_buffer, "test.rs");
    assert_eq!(m.data.input_label, "New file: ");
}

#[test]
fn notification_create_dir_label() {
    let mut m = ExplorerModule::new();
    m.on_notification(
        r#"{"active":true,"rootName":"x","cursorIndex":0,"scrollOffset":0,"width":30,"inputMode":"createDir","inputBuffer":"","showHidden":false,"nodes":[]}"#,
    );
    assert_eq!(m.data.input_label, "New dir: ");
}

#[test]
fn notification_rename_label() {
    let mut m = ExplorerModule::new();
    m.on_notification(
        r#"{"active":true,"rootName":"x","cursorIndex":0,"scrollOffset":0,"width":30,"inputMode":"rename","inputBuffer":"old.rs","showHidden":false,"nodes":[]}"#,
    );
    assert_eq!(m.data.input_label, "Rename: ");
}

#[test]
fn notification_confirm_delete_label() {
    let mut m = ExplorerModule::new();
    m.on_notification(
        r#"{"active":true,"rootName":"x","cursorIndex":0,"scrollOffset":0,"width":30,"inputMode":"confirmDelete","inputBuffer":"","showHidden":false,"nodes":[]}"#,
    );
    assert_eq!(m.data.input_label, "Delete? (y/n): ");
}

#[test]
fn notification_with_message() {
    let mut m = ExplorerModule::new();
    m.on_notification(
        r#"{"active":true,"rootName":"x","cursorIndex":0,"scrollOffset":0,"width":30,"inputMode":"none","inputBuffer":"","showHidden":false,"nodes":[],"message":"File created"}"#,
    );
    assert_eq!(m.data.message.as_deref(), Some("File created"));
}

// =============================================================================
// Cursor position tests
// =============================================================================

#[test]
fn cursor_position_inactive() {
    let m = ExplorerModule::new();
    assert!(m.cursor_position(80, 24).is_none());
}

#[test]
fn cursor_position_active_no_input() {
    let mut m = ExplorerModule::new();
    m.on_notification(&active_payload("proj", &[]));
    assert!(m.cursor_position(80, 24).is_none());
}

#[test]
fn cursor_position_active_with_input() {
    let mut m = ExplorerModule::new();
    m.on_notification(
        r#"{"active":true,"rootName":"x","cursorIndex":0,"scrollOffset":0,"width":30,"inputMode":"createFile","inputBuffer":"ab","showHidden":false,"nodes":[]}"#,
    );
    let pos = m.cursor_position(80, 24);
    assert!(pos.is_some());
    let (x, y) = pos.unwrap();
    // input_label = "New file: " (10 chars), input_buffer = "ab" (2 chars)
    // cursor_x = 1 + 10 + 2 = 13
    assert_eq!(x, 13);
    // input_y = 0 + 24 - 1 = 23
    assert_eq!(y, 23);
}

// =============================================================================
// Render tests
// =============================================================================

#[test]
fn render_inactive_no_op() {
    let m = ExplorerModule::new();
    let mut surface = MockSurface::new(80, 24);
    let bounds = Rect::new(0, 0, 30, 24);
    m.chrome_render(&mut surface, bounds, &test_caps());
    assert!(surface.writes.is_empty());
}

#[test]
fn render_shows_content_with_nodes() {
    let mut m = ExplorerModule::new();
    m.on_notification(&active_payload("project", &[("src", 0, true)]));
    let mut surface = MockSurface::new(80, 24);
    let bounds = Rect::new(0, 0, 30, 24);
    m.chrome_render(&mut surface, bounds, &test_caps());
    // Should have writes (background, header, nodes, separator)
    assert!(!surface.writes.is_empty());
    // Header should contain "project"
    let has_header = surface
        .writes
        .iter()
        .any(|(_, _, text, _)| text == "project");
    assert!(has_header, "Expected header 'project' in writes");
}

#[test]
fn render_at_offset_bounds() {
    let mut m = ExplorerModule::new();
    m.on_notification(&active_payload("proj", &[("file.rs", 0, false)]));
    let mut surface = MockSurface::new(80, 24);
    // Simulate chrome allocating at x=5, y=2
    let bounds = Rect::new(5, 2, 30, 20);
    m.chrome_render(&mut surface, bounds, &test_caps());
    // Header write should be at x=6 (bounds.x + 1), y=2 (bounds.y)
    let has_header_at_offset = surface
        .writes
        .iter()
        .any(|(x, y, text, _)| *x == 6 && *y == 2 && text == "proj");
    assert!(has_header_at_offset, "Expected header at offset (6, 2) in writes");
}

// =============================================================================
// Debug trait tests
// =============================================================================

#[test]
fn node_data_debug() {
    let node = NodeData {
        name: "test".to_owned(),
        depth: 0,
        is_dir: false,
        is_expanded: false,
        is_hidden: false,
        is_last: true,
        vertical_lines: vec![],
        is_symlink: false,
        size: 100,
    };
    let debug = format!("{node:?}");
    assert!(debug.contains("test"));
}

#[test]
fn explorer_data_debug() {
    let data = ExplorerData::default();
    let debug = format!("{data:?}");
    assert!(debug.contains("ExplorerData"));
}
