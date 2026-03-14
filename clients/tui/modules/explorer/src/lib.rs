//! File explorer sidebar native module for reovim TUI.
//!
//! Renders a sidebar tree view on the left side of the terminal.
//! Receives state from the server via `on_notification()` and
//! renders through `RenderSurface`.
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

mod layout;
mod render;

use reovim_client_driver::{
    ChromePosition, ClientModule, ClientModuleError, ModuleContext, PlatformCapabilities,
    ProbeResult, Rect, RenderSurface, Version,
};

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

/// Explorer sidebar native client module.
///
/// Renders the file tree sidebar when active, using `ChromePosition::Left`.
pub struct ExplorerModule {
    data: ExplorerData,
}

impl ExplorerModule {
    /// Create a new inactive explorer module.
    #[must_use]
    pub fn new() -> Self {
        Self {
            data: ExplorerData::default(),
        }
    }
}

impl Default for ExplorerModule {
    fn default() -> Self {
        Self::new()
    }
}

impl ClientModule for ExplorerModule {
    fn id(&self) -> &'static str {
        "explorer"
    }

    fn kind(&self) -> &'static str {
        "explorer"
    }

    fn name(&self) -> &'static str {
        "Explorer"
    }

    fn version(&self) -> Version {
        Version::new(0, 1, 0)
    }

    fn init(&mut self, _ctx: &ModuleContext) -> ProbeResult {
        ProbeResult::Success
    }

    fn exit(&mut self) -> Result<(), ClientModuleError> {
        Ok(())
    }

    fn has_chrome(&self) -> bool {
        true
    }

    fn chrome_position(&self) -> ChromePosition {
        ChromePosition::Left
    }

    fn chrome_requested_size(&self, _caps: &dyn PlatformCapabilities) -> u16 {
        if self.data.active { self.data.width } else { 0 }
    }

    fn chrome_priority(&self) -> u16 {
        60
    }

    #[allow(clippy::cast_possible_truncation)]
    fn on_notification(&mut self, data: &str) {
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

    #[allow(clippy::cast_possible_truncation)]
    fn cursor_position(&self, _w: u16, h: u16) -> Option<(u16, u16)> {
        if !self.data.active || self.data.input_mode == "none" {
            return None;
        }

        // When in input mode, calculate bounds assuming the sidebar starts at x=0, y=0
        // with the full terminal height. The chrome system positions us at (0, 0).
        let bounds = SidebarBounds::calculate(0, 0, self.data.width, h, true);
        let input_y = bounds.input_y?;

        // Cursor at end of input label + input buffer
        let cursor_x = 1 + self.data.input_label.len() as u16 + self.data.input_buffer.len() as u16;
        Some((cursor_x.min(self.data.width.saturating_sub(2)), input_y))
    }

    fn chrome_render(
        &self,
        surface: &mut dyn RenderSurface,
        bounds: Rect,
        _caps: &dyn PlatformCapabilities,
    ) {
        if !self.data.active {
            return;
        }

        let has_input = self.data.input_mode != "none";
        let sidebar_bounds =
            SidebarBounds::calculate(bounds.x, bounds.y, bounds.width, bounds.height, has_input);
        render::render_explorer(surface, &self.data, &sidebar_bounds);
    }
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
