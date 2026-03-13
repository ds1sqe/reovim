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
const KIND: &str = "explorer";

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
        KIND
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
#[path = "lib_tests.rs"]
mod tests;
