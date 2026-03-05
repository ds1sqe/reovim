#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! File explorer sidebar TUI extension.
//!
//! Renders a sidebar tree view on the left side of the terminal.
//! Receives state from the server via `apply_notification()` and
//! renders through `RenderBackend`.
//!
//! # Layout
//!
//! ```text
//! +--------+------------------------------------------+
//! | project|  Editor Content                          |
//! |--------|                                          |
//! | v src/ |  fn main() {                             |
//! |   main |      println!("hello");                  |
//! |   lib  |  }                                       |
//! | > test |                                          |
//! | readme |                                          |
//! +--------+------------------------------------------+
//! ```

pub mod layout;
pub mod render;

use reovim_driver_display::render_backend::{RenderBackend, TuiExtension};

use crate::layout::SidebarBounds;

/// Deserialized explorer state from server notification.
#[derive(Debug, Default)]
pub(crate) struct ExplorerData {
    pub active: bool,
    pub root_name: String,
    pub cursor_index: usize,
    pub scroll_offset: usize,
    pub width: u16,
    pub input_mode: String,
    pub input_buffer: String,
    pub input_label: String,
    pub message: Option<String>,
    pub show_hidden: bool,
    pub nodes: Vec<NodeData>,
}

/// Deserialized tree node data.
#[derive(Debug)]
#[allow(clippy::struct_excessive_bools)]
pub(crate) struct NodeData {
    pub name: String,
    pub depth: usize,
    pub is_dir: bool,
    pub is_expanded: bool,
    pub is_hidden: bool,
    pub is_last: bool,
    pub vertical_lines: Vec<bool>,
    pub is_symlink: bool,
    #[allow(dead_code)] // Reserved for file details popup
    pub size: u64,
}

/// Explorer sidebar TUI extension.
///
/// Renders the file tree sidebar when active.
pub struct ExplorerExtension {
    data: ExplorerData,
}

impl ExplorerExtension {
    /// Create a new inactive extension.
    #[must_use]
    pub fn new() -> Self {
        Self {
            data: ExplorerData::default(),
        }
    }
}

impl Default for ExplorerExtension {
    fn default() -> Self {
        Self::new()
    }
}

impl TuiExtension for ExplorerExtension {
    fn kind(&self) -> &'static str {
        "explorer"
    }

    fn is_active(&self) -> bool {
        self.data.active
    }

    #[allow(clippy::cast_possible_truncation)]
    fn apply_notification(&mut self, data: &str) {
        let Ok(json) = serde_json::from_str::<serde_json::Value>(data) else {
            return;
        };

        let active = json["active"].as_bool().unwrap_or(false);
        if !active {
            self.data.active = false;
            return;
        }

        self.data.active = true;
        json["rootName"]
            .as_str()
            .unwrap_or("")
            .clone_into(&mut self.data.root_name);
        self.data.cursor_index = json["cursorIndex"].as_u64().unwrap_or(0) as usize;
        self.data.scroll_offset = json["scrollOffset"].as_u64().unwrap_or(0) as usize;
        self.data.width = json["width"].as_u64().unwrap_or(30) as u16;
        json["inputMode"]
            .as_str()
            .unwrap_or("none")
            .clone_into(&mut self.data.input_mode);
        json["inputBuffer"]
            .as_str()
            .unwrap_or("")
            .clone_into(&mut self.data.input_buffer);
        self.data.show_hidden = json["showHidden"].as_bool().unwrap_or(false);
        self.data.message = json["message"].as_str().map(str::to_owned);

        // Derive input label from input mode
        self.data.input_label = match self.data.input_mode.as_str() {
            "createFile" => "New file: ".to_owned(),
            "createDir" => "New dir: ".to_owned(),
            "rename" => "Rename: ".to_owned(),
            "confirmDelete" => "Delete? (y/n): ".to_owned(),
            _ => String::new(),
        };

        // Delta snapshot support: if the server omits "nodes", keep the
        // existing node data (only metadata like cursorIndex changed).
        if let Some(arr) = json["nodes"].as_array() {
            self.data.nodes = arr
                .iter()
                .map(|item| NodeData {
                    name: item["name"].as_str().unwrap_or("").to_owned(),
                    depth: item["depth"].as_u64().unwrap_or(0) as usize,
                    is_dir: item["isDir"].as_bool().unwrap_or(false),
                    is_expanded: item["isExpanded"].as_bool().unwrap_or(false),
                    is_hidden: item["isHidden"].as_bool().unwrap_or(false),
                    is_last: item["isLast"].as_bool().unwrap_or(false),
                    vertical_lines: item["verticalLines"]
                        .as_array()
                        .map(|a| a.iter().filter_map(serde_json::Value::as_bool).collect())
                        .unwrap_or_default(),
                    is_symlink: item["isSymlink"].as_bool().unwrap_or(false),
                    size: item["size"].as_u64().unwrap_or(0),
                })
                .collect();
        }
    }

    fn render(&self, backend: &mut dyn RenderBackend) {
        let (_, height) = backend.size();
        let has_input = self.data.input_mode != "none";
        let bounds = SidebarBounds::calculate(self.data.width, height, has_input);
        render::render_explorer(backend, &self.data, &bounds);
    }

    fn content_offset_left(&self) -> u16 {
        if self.data.active { self.data.width } else { 0 }
    }

    #[allow(clippy::cast_possible_truncation)]
    fn cursor_position(&self, _terminal_width: u16, terminal_height: u16) -> Option<(u16, u16)> {
        if !self.data.active || self.data.input_mode == "none" {
            return None;
        }

        let bounds = SidebarBounds::calculate(self.data.width, terminal_height, true);
        let input_y = bounds.input_y?;

        // Cursor at end of input label + input buffer
        let cursor_x = 1 + self.data.input_label.len() as u16 + self.data.input_buffer.len() as u16;
        Some((cursor_x.min(self.data.width.saturating_sub(2)), input_y))
    }
}

#[cfg(test)]
mod tests {
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
}
