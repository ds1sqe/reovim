//! Shared types for protocol messages.
//!
//! This module defines serializable types that represent editor state.
//! These are "snapshot" types designed for wire transmission, not
//! runtime manipulation.

use serde::{Deserialize, Serialize};

/// Cursor/text position in a buffer.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct Position {
    /// Line number (0-indexed).
    pub line: usize,
    /// Column number (0-indexed, byte offset).
    pub column: usize,
}

impl Position {
    /// Create a new position.
    #[must_use]
    pub const fn new(line: usize, column: usize) -> Self {
        Self { line, column }
    }
}

/// Buffer identifier.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct BufferId(pub usize);

impl From<usize> for BufferId {
    fn from(id: usize) -> Self {
        Self(id)
    }
}

impl From<BufferId> for usize {
    fn from(id: BufferId) -> Self {
        id.0
    }
}

/// Window identifier.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct WindowId(pub usize);

impl From<usize> for WindowId {
    fn from(id: usize) -> Self {
        Self(id)
    }
}

impl From<WindowId> for usize {
    fn from(id: WindowId) -> Self {
        id.0
    }
}

/// Selection mode.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SelectionMode {
    /// Character-wise selection (like vim's `v`).
    #[default]
    Character,
    /// Line-wise selection (like vim's `V`).
    Line,
    /// Block/rectangular selection (like vim's `Ctrl-V`).
    Block,
}

/// Screen content format for rendering.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ScreenFormat {
    /// Raw terminal output with ANSI escape codes.
    RawAnsi,
    /// Plain text with ANSI codes stripped.
    #[default]
    PlainText,
    /// Structured cell grid as JSON.
    CellGrid,
}

/// Mode information snapshot.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ModeInfo {
    /// Focus area (e.g., "Editor", "Explorer").
    pub focus: String,
    /// Edit mode (e.g., "Normal", "Insert", "Visual").
    pub edit_mode: String,
    /// Sub-mode (e.g., "None", "Command", "`OperatorPending`").
    pub sub_mode: String,
    /// Display string for statusline.
    pub display: String,
}

/// Cursor information snapshot.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CursorInfo {
    /// Line number (0-indexed).
    pub line: usize,
    /// Column number (0-indexed).
    pub column: usize,
}

impl From<Position> for CursorInfo {
    fn from(pos: Position) -> Self {
        Self {
            line: pos.line,
            column: pos.column,
        }
    }
}

impl From<CursorInfo> for Position {
    fn from(info: CursorInfo) -> Self {
        Self {
            line: info.line,
            column: info.column,
        }
    }
}

/// Selection information snapshot.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SelectionInfo {
    /// Whether selection is active.
    pub active: bool,
    /// Selection mode.
    pub mode: SelectionMode,
    /// Anchor position (where selection started).
    pub anchor: Position,
    /// Cursor position (current end of selection).
    pub cursor: Position,
}

/// Buffer information snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BufferInfo {
    /// Buffer ID.
    pub id: usize,
    /// File path (if associated with a file).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_path: Option<String>,
    /// Whether the buffer has unsaved changes.
    pub modified: bool,
    /// Total line count.
    pub line_count: usize,
}

/// Screen information snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScreenInfo {
    /// Terminal width in columns.
    pub width: u16,
    /// Terminal height in rows.
    pub height: u16,
    /// Currently active buffer ID.
    pub active_buffer_id: BufferId,
    /// Currently active window ID (if any).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_window_id: Option<WindowId>,
    /// Total number of windows.
    pub window_count: usize,
}

/// Window information snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowInfo {
    /// Window ID.
    pub id: WindowId,
    /// Buffer displayed in this window.
    pub buffer_id: BufferId,
    /// Whether this window is active.
    pub is_active: bool,
    /// Cursor position within the window.
    pub cursor: Position,
}

/// Cell information for grid rendering.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CellInfo {
    /// Character to display.
    pub char: char,
    /// Foreground color (CSS hex like "#ff0000" or "ansi:9").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fg: Option<String>,
    /// Background color (CSS hex like "#ff0000" or "ansi:9").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bg: Option<String>,
    /// Bold attribute.
    #[serde(default, skip_serializing_if = "is_false")]
    pub bold: bool,
    /// Italic attribute.
    #[serde(default, skip_serializing_if = "is_false")]
    pub italic: bool,
    /// Underline attribute.
    #[serde(default, skip_serializing_if = "is_false")]
    pub underline: bool,
}

/// Helper for serde's `skip_serializing_if`.
/// Note: serde requires `&T` signature, hence the reference.
#[allow(clippy::trivially_copy_pass_by_ref)]
const fn is_false(b: &bool) -> bool {
    !*b
}

impl Default for CellInfo {
    fn default() -> Self {
        Self {
            char: ' ',
            fg: None,
            bg: None,
            bold: false,
            italic: false,
            underline: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_position_serialization() {
        let pos = Position::new(10, 5);
        let json = serde_json::to_string(&pos).unwrap();
        assert_eq!(json, r#"{"line":10,"column":5}"#);
    }

    #[test]
    fn test_selection_mode_serialization() {
        assert_eq!(serde_json::to_string(&SelectionMode::Character).unwrap(), "\"character\"");
        assert_eq!(serde_json::to_string(&SelectionMode::Line).unwrap(), "\"line\"");
        assert_eq!(serde_json::to_string(&SelectionMode::Block).unwrap(), "\"block\"");
    }

    #[test]
    fn test_screen_format_serialization() {
        assert_eq!(serde_json::to_string(&ScreenFormat::RawAnsi).unwrap(), "\"raw_ansi\"");
        assert_eq!(serde_json::to_string(&ScreenFormat::PlainText).unwrap(), "\"plain_text\"");
        assert_eq!(serde_json::to_string(&ScreenFormat::CellGrid).unwrap(), "\"cell_grid\"");
    }

    #[test]
    fn test_buffer_info_serialization() {
        let buffer = BufferInfo {
            id: 1,
            file_path: Some("/tmp/test.txt".to_string()),
            modified: true,
            line_count: 100,
        };
        let json = serde_json::to_string(&buffer).unwrap();
        assert!(json.contains("\"id\":1"));
        assert!(json.contains("\"file_path\":\"/tmp/test.txt\""));
        assert!(json.contains("\"modified\":true"));
    }

    #[test]
    fn test_cell_info_skips_defaults() {
        let cell = CellInfo {
            char: 'a',
            fg: Some("#ff0000".to_string()),
            bg: None,
            bold: false,
            italic: false,
            underline: false,
        };
        let json = serde_json::to_string(&cell).unwrap();
        // Should skip false booleans and None
        assert!(!json.contains("\"bold\""));
        assert!(!json.contains("\"bg\""));
        assert!(json.contains("\"fg\":\"#ff0000\""));
    }
}
