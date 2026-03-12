use super::*;

#[test]
fn extension_kind() {
    let ext = ExplorerExtension::new();
    assert_eq!(ext.kind(), "explorer");
}

#[test]
fn initially_inactive() {
    let ext = ExplorerExtension::new();
    assert!(!ext.is_active());
}

#[test]
fn default_impl() {
    let ext = ExplorerExtension::default();
    assert!(!ext.is_active());
}

#[test]
fn content_offset_inactive() {
    let ext = ExplorerExtension::new();
    assert_eq!(ext.content_offset_left(), 0);
}

#[test]
fn content_offset_active() {
    let mut ext = ExplorerExtension::new();
    ext.apply_notification(
        r#"{"active":true,"rootName":"project","cursorIndex":0,"scrollOffset":0,"width":30,"inputMode":"none","inputBuffer":"","showHidden":false,"nodes":[]}"#,
    );
    assert_eq!(ext.content_offset_left(), 30);
}

#[test]
fn apply_notification_active() {
    let mut ext = ExplorerExtension::new();
    ext.apply_notification(
        r#"{"active":true,"rootName":"my-project","cursorIndex":2,"scrollOffset":1,"width":35,"inputMode":"none","inputBuffer":"","showHidden":true,"nodes":[{"name":"src","depth":0,"isDir":true,"isExpanded":true,"isHidden":false,"isLast":false,"verticalLines":[],"isSymlink":false,"size":0}]}"#,
    );
    assert!(ext.is_active());
    assert_eq!(ext.data.root_name, "my-project");
    assert_eq!(ext.data.cursor_index, 2);
    assert_eq!(ext.data.scroll_offset, 1);
    assert_eq!(ext.data.width, 35);
    assert!(ext.data.show_hidden);
    assert_eq!(ext.data.nodes.len(), 1);
    assert_eq!(ext.data.nodes[0].name, "src");
    assert!(ext.data.nodes[0].is_dir);
}

#[test]
fn apply_notification_inactive() {
    let mut ext = ExplorerExtension::new();
    ext.apply_notification(
        r#"{"active":true,"rootName":"x","cursorIndex":0,"scrollOffset":0,"width":30,"inputMode":"none","inputBuffer":"","showHidden":false,"nodes":[]}"#,
    );
    assert!(ext.is_active());

    ext.apply_notification(r#"{"active":false}"#);
    assert!(!ext.is_active());
}

#[test]
fn apply_notification_invalid_json() {
    let mut ext = ExplorerExtension::new();
    ext.apply_notification("not json");
    assert!(!ext.is_active());
}

#[test]
fn apply_notification_with_message() {
    let mut ext = ExplorerExtension::new();
    ext.apply_notification(
        r#"{"active":true,"rootName":"x","cursorIndex":0,"scrollOffset":0,"width":30,"inputMode":"none","inputBuffer":"","showHidden":false,"nodes":[],"message":"File created"}"#,
    );
    assert_eq!(ext.data.message.as_deref(), Some("File created"));
}

#[test]
fn apply_notification_input_mode() {
    let mut ext = ExplorerExtension::new();
    ext.apply_notification(
        r#"{"active":true,"rootName":"x","cursorIndex":0,"scrollOffset":0,"width":30,"inputMode":"createFile","inputBuffer":"test.rs","showHidden":false,"nodes":[]}"#,
    );
    assert_eq!(ext.data.input_mode, "createFile");
    assert_eq!(ext.data.input_buffer, "test.rs");
    assert_eq!(ext.data.input_label, "New file: ");
}

#[test]
fn apply_notification_rename_label() {
    let mut ext = ExplorerExtension::new();
    ext.apply_notification(
        r#"{"active":true,"rootName":"x","cursorIndex":0,"scrollOffset":0,"width":30,"inputMode":"rename","inputBuffer":"old.rs","showHidden":false,"nodes":[]}"#,
    );
    assert_eq!(ext.data.input_label, "Rename: ");
}

#[test]
fn apply_notification_create_dir_label() {
    let mut ext = ExplorerExtension::new();
    ext.apply_notification(
        r#"{"active":true,"rootName":"x","cursorIndex":0,"scrollOffset":0,"width":30,"inputMode":"createDir","inputBuffer":"","showHidden":false,"nodes":[]}"#,
    );
    assert_eq!(ext.data.input_label, "New dir: ");
}

#[test]
fn apply_notification_confirm_delete_label() {
    let mut ext = ExplorerExtension::new();
    ext.apply_notification(
        r#"{"active":true,"rootName":"x","cursorIndex":0,"scrollOffset":0,"width":30,"inputMode":"confirmDelete","inputBuffer":"","showHidden":false,"nodes":[]}"#,
    );
    assert_eq!(ext.data.input_label, "Delete? (y/n): ");
}

#[test]
fn cursor_position_inactive() {
    let ext = ExplorerExtension::new();
    assert!(ext.cursor_position(80, 24).is_none());
}

#[test]
fn cursor_position_browse_mode() {
    let mut ext = ExplorerExtension::new();
    ext.apply_notification(
        r#"{"active":true,"rootName":"x","cursorIndex":0,"scrollOffset":0,"width":30,"inputMode":"none","inputBuffer":"","showHidden":false,"nodes":[]}"#,
    );
    assert!(ext.cursor_position(80, 24).is_none());
}

#[test]
fn cursor_position_input_mode() {
    let mut ext = ExplorerExtension::new();
    ext.apply_notification(
        r#"{"active":true,"rootName":"x","cursorIndex":0,"scrollOffset":0,"width":30,"inputMode":"createFile","inputBuffer":"ab","showHidden":false,"nodes":[]}"#,
    );
    let pos = ext.cursor_position(80, 24);
    assert!(pos.is_some());
}

#[test]
fn render_does_not_panic() {
    use reovim_driver_display::FrameBuffer;

    let mut ext = ExplorerExtension::new();
    ext.apply_notification(
        r#"{"active":true,"rootName":"project","cursorIndex":0,"scrollOffset":0,"width":30,"inputMode":"none","inputBuffer":"","showHidden":false,"nodes":[{"name":"src","depth":0,"isDir":true,"isExpanded":false,"isHidden":false,"isLast":true,"verticalLines":[],"isSymlink":false,"size":0}]}"#,
    );
    let mut fb = FrameBuffer::new(80, 24);
    ext.render(&mut fb);
    // Verify header was rendered
    assert_eq!(fb.get(1, 0).map(|c| c.char), Some('p'));
}

#[test]
fn apply_delta_notification_keeps_existing_nodes() {
    let mut ext = ExplorerExtension::new();

    // First: full notification with nodes
    ext.apply_notification(
        r#"{"active":true,"rootName":"proj","cursorIndex":0,"scrollOffset":0,"width":30,"inputMode":"none","inputBuffer":"","showHidden":false,"nodes":[{"name":"src","depth":0,"isDir":true,"isExpanded":false,"isHidden":false,"isLast":true,"verticalLines":[],"isSymlink":false,"size":0}]}"#,
    );
    assert_eq!(ext.data.nodes.len(), 1);
    assert_eq!(ext.data.nodes[0].name, "src");

    // Second: delta notification (no "nodes" key), cursor moved
    ext.apply_notification(
        r#"{"active":true,"rootName":"proj","cursorIndex":5,"scrollOffset":2,"width":30,"inputMode":"none","inputBuffer":"","showHidden":false}"#,
    );
    // Nodes preserved from previous notification
    assert_eq!(ext.data.nodes.len(), 1);
    assert_eq!(ext.data.nodes[0].name, "src");
    // Metadata updated
    assert_eq!(ext.data.cursor_index, 5);
    assert_eq!(ext.data.scroll_offset, 2);
}

#[test]
fn apply_full_notification_replaces_nodes() {
    let mut ext = ExplorerExtension::new();

    // Full notification with one node
    ext.apply_notification(
        r#"{"active":true,"rootName":"proj","cursorIndex":0,"scrollOffset":0,"width":30,"inputMode":"none","inputBuffer":"","showHidden":false,"nodes":[{"name":"old","depth":0,"isDir":false,"isExpanded":false,"isHidden":false,"isLast":true,"verticalLines":[],"isSymlink":false,"size":0}]}"#,
    );
    assert_eq!(ext.data.nodes.len(), 1);
    assert_eq!(ext.data.nodes[0].name, "old");

    // Full notification with different nodes
    ext.apply_notification(
        r#"{"active":true,"rootName":"proj","cursorIndex":0,"scrollOffset":0,"width":30,"inputMode":"none","inputBuffer":"","showHidden":false,"nodes":[{"name":"new1","depth":0,"isDir":true,"isExpanded":true,"isHidden":false,"isLast":false,"verticalLines":[],"isSymlink":false,"size":0},{"name":"new2","depth":1,"isDir":false,"isExpanded":false,"isHidden":false,"isLast":true,"verticalLines":[true],"isSymlink":false,"size":42}]}"#,
    );
    assert_eq!(ext.data.nodes.len(), 2);
    assert_eq!(ext.data.nodes[0].name, "new1");
    assert_eq!(ext.data.nodes[1].name, "new2");
}

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
