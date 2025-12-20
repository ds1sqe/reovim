//! State snapshot types for RPC serialization
//!
//! These types provide serializable snapshots of editor state
//! for external tools to query and monitor.

use serde::{Deserialize, Serialize};

/// Snapshot of the editor mode state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModeSnapshot {
    /// Focus context (Editor, Explorer, Telescope)
    pub focus: String,
    /// Edit mode (Normal, Insert, Visual)
    pub edit_mode: String,
    /// Sub-mode (None, Command, `OperatorPending`, Leap)
    pub sub_mode: String,
    /// Human-readable display string (e.g., " NORMAL ")
    pub display: String,
}

/// Snapshot of cursor position
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CursorSnapshot {
    /// Column (0-indexed)
    pub x: u16,
    /// Row (0-indexed)
    pub y: u16,
}

/// Snapshot of buffer metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BufferSnapshot {
    /// Buffer ID
    pub id: usize,
    /// File path (if any)
    pub file_path: Option<String>,
    /// Whether buffer has unsaved changes
    pub modified: bool,
    /// Number of lines in buffer
    pub line_count: usize,
}

/// Snapshot of visual selection state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SelectionSnapshot {
    /// Whether selection is active
    pub active: bool,
    /// Selection mode (Character, Block, Line)
    pub mode: String,
    /// Anchor position (where selection started)
    pub anchor: CursorSnapshot,
    /// Current cursor position
    pub cursor: CursorSnapshot,
}

/// Snapshot of screen dimensions and state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScreenSnapshot {
    /// Screen width in columns
    pub width: u16,
    /// Screen height in rows
    pub height: u16,
    /// ID of the active buffer
    pub active_buffer_id: usize,
    /// ID of the active window (if any)
    pub active_window_id: Option<usize>,
    /// Number of windows in the active tab
    pub window_count: usize,
}

/// Snapshot of a single window's state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowSnapshot {
    /// Window ID
    pub id: usize,
    /// Buffer ID displayed in this window
    pub buffer_id: usize,
    /// Horizontal scroll offset (buffer column at left edge)
    pub buffer_anchor_x: u16,
    /// Vertical scroll offset (buffer line at top edge)
    pub buffer_anchor_y: u16,
    /// Whether this window is currently active
    pub is_active: bool,
    /// Saved cursor X position (for inactive windows)
    pub cursor_x: u16,
    /// Saved cursor Y position (for inactive windows)
    pub cursor_y: u16,
}

/// Format for screen content
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ScreenFormat {
    /// Raw terminal output with ANSI escape codes
    RawAnsi,
    /// Plain text with ANSI codes stripped
    #[default]
    PlainText,
    /// Cell grid representation (char + style per cell)
    CellGrid,
}

/// Snapshot of full screen content
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScreenContentSnapshot {
    /// Screen width
    pub width: u16,
    /// Screen height
    pub height: u16,
    /// Content format
    pub format: ScreenFormat,
    /// Screen content (format depends on `format` field)
    /// - `RawAnsi`: Raw bytes with ANSI codes
    /// - `PlainText`: Human-readable text
    /// - `CellGrid`: JSON array of cell rows
    pub content: String,
}

/// Individual cell in cell grid format
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CellSnapshot {
    /// Character at this cell
    pub char: char,
    /// Foreground color (CSS-style hex, e.g., "#ff0000")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fg: Option<String>,
    /// Background color
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bg: Option<String>,
    /// Bold style
    #[serde(default, skip_serializing_if = "is_false")]
    pub bold: bool,
    /// Italic style
    #[serde(default, skip_serializing_if = "is_false")]
    pub italic: bool,
    /// Underline style
    #[serde(default, skip_serializing_if = "is_false")]
    pub underline: bool,
}

#[allow(clippy::trivially_copy_pass_by_ref)]
const fn is_false(b: &bool) -> bool {
    !*b
}

impl CellSnapshot {
    /// Create a new cell with just a character
    #[must_use]
    pub const fn new(char: char) -> Self {
        Self {
            char,
            fg: None,
            bg: None,
            bold: false,
            italic: false,
            underline: false,
        }
    }
}

// === Conversion implementations ===

impl From<&crate::modd::ModeState> for ModeSnapshot {
    fn from(mode: &crate::modd::ModeState) -> Self {
        // Use the interactor_id field
        let focus = mode.interactor_id.0;

        let edit_mode = match &mode.edit_mode {
            crate::modd::EditMode::Normal => "Normal".to_string(),
            crate::modd::EditMode::Insert(ext) => format!("Insert({ext:?})"),
            crate::modd::EditMode::Visual(ext) => format!("Visual({ext:?})"),
        };

        let sub_mode = match &mode.sub_mode {
            crate::modd::SubMode::None => "None".to_string(),
            crate::modd::SubMode::Command => "Command".to_string(),
            crate::modd::SubMode::OperatorPending { operator, count } => {
                format!("OperatorPending({operator:?}, {count:?})")
            }
            crate::modd::SubMode::Leap {
                direction,
                operator,
                count,
            } => {
                format!("Leap({direction:?}, {operator:?}, {count:?})")
            }
        };

        Self {
            focus: focus.to_string(),
            edit_mode,
            sub_mode,
            display: mode.display_string().to_string(),
        }
    }
}

impl From<&crate::screen::Position> for CursorSnapshot {
    fn from(pos: &crate::screen::Position) -> Self {
        Self { x: pos.x, y: pos.y }
    }
}

impl From<crate::screen::Position> for CursorSnapshot {
    fn from(pos: crate::screen::Position) -> Self {
        Self { x: pos.x, y: pos.y }
    }
}

impl From<&crate::buffer::Buffer> for BufferSnapshot {
    fn from(buffer: &crate::buffer::Buffer) -> Self {
        Self {
            id: buffer.id,
            file_path: buffer.file_path.clone(),
            modified: buffer.modified,
            line_count: buffer.contents.len(),
        }
    }
}

/// Snapshot of which-key panel state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WhichKeySnapshot {
    /// Whether the panel is visible
    pub visible: bool,
    /// Current prefix being shown
    pub prefix: String,
    /// Available bindings for the current prefix
    pub bindings: Vec<WhichKeyBindingSnapshot>,
}

/// Individual binding in which-key panel
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WhichKeyBindingSnapshot {
    /// The key sequence (e.g., "g" or "gg")
    pub key: String,
    /// Description of what this key does
    pub description: String,
    /// Whether this is a prefix (has more bindings) or a terminal command
    pub is_prefix: bool,
}

impl From<&crate::screen::WhichKeyPanel> for WhichKeySnapshot {
    fn from(panel: &crate::screen::WhichKeyPanel) -> Self {
        Self {
            visible: panel.visible,
            prefix: panel.prefix.clone(),
            bindings: panel
                .bindings
                .iter()
                .map(WhichKeyBindingSnapshot::from)
                .collect(),
        }
    }
}

impl From<&crate::event::WhichKeyBinding> for WhichKeyBindingSnapshot {
    fn from(binding: &crate::event::WhichKeyBinding) -> Self {
        Self {
            key: binding.key.clone(),
            description: binding.description.clone(),
            is_prefix: binding.is_prefix,
        }
    }
}

/// Snapshot of telescope state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelescopeSnapshot {
    /// Whether telescope is currently active/visible
    pub active: bool,
    /// Current search query
    pub query: String,
    /// Currently selected item index
    pub selected_index: usize,
    /// Number of items in results
    pub item_count: usize,
    /// Name of the current picker
    pub picker_name: String,
    /// Title displayed
    pub title: String,
    /// Selected item display text (if any)
    pub selected_item: Option<String>,
}

impl From<&crate::telescope::TelescopeState> for TelescopeSnapshot {
    fn from(state: &crate::telescope::TelescopeState) -> Self {
        Self {
            active: state.active,
            query: state.query.clone(),
            selected_index: state.selected_index,
            item_count: state.items.len(),
            picker_name: state.picker_name.clone(),
            title: state.title.clone(),
            selected_item: state
                .items
                .get(state.selected_index)
                .map(|i| i.display.clone()),
        }
    }
}

impl From<&crate::buffer::Selection> for SelectionSnapshot {
    fn from(sel: &crate::buffer::Selection) -> Self {
        let mode = match sel.mode {
            crate::buffer::SelectionMode::Character => "Character",
            crate::buffer::SelectionMode::Block => "Block",
            crate::buffer::SelectionMode::Line => "Line",
        };

        Self {
            active: sel.active,
            mode: mode.to_string(),
            anchor: CursorSnapshot::from(&sel.anchor),
            cursor: CursorSnapshot { x: 0, y: 0 }, // Cursor is set separately from buffer
        }
    }
}

// === Color conversion ===

/// Convert a Color to a CSS-style hex string
#[must_use]
pub fn color_to_hex(color: &reovim_sys::style::Color) -> String {
    use reovim_sys::style::Color;
    match color {
        Color::Rgb { r, g, b } => format!("#{r:02x}{g:02x}{b:02x}"),
        Color::AnsiValue(n) => format!("ansi:{n}"),
        Color::Black => "#000000".to_string(),
        Color::DarkGrey => "#808080".to_string(),
        Color::Red => "#ff0000".to_string(),
        Color::DarkRed => "#800000".to_string(),
        Color::Green => "#00ff00".to_string(),
        Color::DarkGreen => "#008000".to_string(),
        Color::Yellow => "#ffff00".to_string(),
        Color::DarkYellow => "#808000".to_string(),
        Color::Blue => "#0000ff".to_string(),
        Color::DarkBlue => "#000080".to_string(),
        Color::Magenta => "#ff00ff".to_string(),
        Color::DarkMagenta => "#800080".to_string(),
        Color::Cyan => "#00ffff".to_string(),
        Color::DarkCyan => "#008080".to_string(),
        Color::White => "#ffffff".to_string(),
        Color::Grey => "#c0c0c0".to_string(),
        Color::Reset => "reset".to_string(),
    }
}

// === Cell conversion ===

impl From<&crate::frame::Cell> for CellSnapshot {
    fn from(cell: &crate::frame::Cell) -> Self {
        use crate::highlight::Attributes;

        Self {
            char: cell.char,
            fg: cell.style.fg.as_ref().map(color_to_hex),
            bg: cell.style.bg.as_ref().map(color_to_hex),
            bold: cell.style.attributes.contains(Attributes::BOLD),
            italic: cell.style.attributes.contains(Attributes::ITALIC),
            underline: cell.style.attributes.contains(Attributes::UNDERLINE),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mode_snapshot_serialization() {
        let snapshot = ModeSnapshot {
            focus: "Editor".to_string(),
            edit_mode: "Normal".to_string(),
            sub_mode: "None".to_string(),
            display: " NORMAL ".to_string(),
        };
        let json = serde_json::to_string(&snapshot).unwrap();
        assert!(json.contains("\"focus\":\"Editor\""));
    }

    #[test]
    fn test_cursor_snapshot_serialization() {
        let snapshot = CursorSnapshot { x: 10, y: 5 };
        let json = serde_json::to_string(&snapshot).unwrap();
        assert_eq!(json, r#"{"x":10,"y":5}"#);
    }

    #[test]
    fn test_screen_format_deserialization() {
        let format: ScreenFormat = serde_json::from_str(r#""plain_text""#).unwrap();
        assert!(matches!(format, ScreenFormat::PlainText));

        let format: ScreenFormat = serde_json::from_str(r#""raw_ansi""#).unwrap();
        assert!(matches!(format, ScreenFormat::RawAnsi));
    }

    #[test]
    fn test_cell_snapshot_omits_defaults() {
        let cell = CellSnapshot::new('A');
        let json = serde_json::to_string(&cell).unwrap();
        // Should only have "char", not bold/italic/underline
        assert_eq!(json, r#"{"char":"A"}"#);
    }
}
