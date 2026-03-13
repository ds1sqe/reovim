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

/// Wire-format window identifier for serialization.
///
/// This is a thin wrapper for protocol serialization only.
/// Convert to/from kernel `WindowId` at RPC boundaries in the server layer.
///
/// `#[serde(transparent)]` ensures wire format is just `42`, not `{"0": 42}`.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(transparent)]
pub struct WireWindowId(pub usize);

impl From<usize> for WireWindowId {
    fn from(id: usize) -> Self {
        Self(id)
    }
}

impl From<WireWindowId> for usize {
    fn from(id: WireWindowId) -> Self {
        id.0
    }
}

/// Wire-format layer identifier for serialization.
///
/// Represents a compositor layer in the protocol.
/// `#[serde(transparent)]` ensures wire format is just `42`, not `{"0": 42}`.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(transparent)]
pub struct WireLayerId(pub usize);

impl From<usize> for WireLayerId {
    fn from(id: usize) -> Self {
        Self(id)
    }
}

impl From<WireLayerId> for usize {
    fn from(id: WireLayerId) -> Self {
        id.0
    }
}

/// Rectangle bounds for layout geometry.
///
/// Represents a window's position and size in terminal cells.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct WireRect {
    /// X position (column, 0-indexed).
    pub x: u16,
    /// Y position (row, 0-indexed).
    pub y: u16,
    /// Width in columns.
    pub width: u16,
    /// Height in rows.
    pub height: u16,
}

impl WireRect {
    /// Create a new rectangle.
    #[must_use]
    pub const fn new(x: u16, y: u16, width: u16, height: u16) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    /// Create a rectangle covering the full screen.
    #[must_use]
    pub const fn full_screen(width: u16, height: u16) -> Self {
        Self {
            x: 0,
            y: 0,
            width,
            height,
        }
    }
}

/// Zone type within a compositor layer.
///
/// Each layer has three zones with different stacking behavior.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum WireZone {
    /// Tiled zone - windows arranged in binary split tree.
    #[default]
    Tiled,
    /// Float zone - freely positioned floating windows.
    Float,
    /// Overlay zone - popups, menus, tooltips.
    Overlay,
}

/// Split direction for window splits.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WireSplitDirection {
    /// Horizontal split (windows stacked vertically).
    Horizontal,
    /// Vertical split (windows side by side).
    Vertical,
}

/// Type of layout change that occurred.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum WireLayoutChangeKind {
    /// Window was split.
    Split {
        /// ID of the new window created by split.
        new_window: WireWindowId,
        /// Direction of the split.
        direction: WireSplitDirection,
    },
    /// Window was closed.
    Close {
        /// ID of the closed window.
        closed_window: WireWindowId,
        /// ID of the window that received focus (if any).
        new_focus: Option<WireWindowId>,
    },
    /// Focus changed to a different window.
    Focus {
        /// Previous focused window (if any).
        from: Option<WireWindowId>,
        /// New focused window.
        to: WireWindowId,
    },
    /// Window was resized.
    Resize {
        /// ID of the resized window.
        window: WireWindowId,
    },
    /// All windows were equalized.
    Equalize,
}

/// Window placement information in the layout.
///
/// Contains all information needed to render a window.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WireWindowPlacement {
    /// Window identifier.
    pub window_id: WireWindowId,
    /// Layer this window belongs to.
    pub layer_id: WireLayerId,
    /// Zone within the layer.
    pub zone: WireZone,
    /// Screen bounds (position and size).
    pub bounds: WireRect,
    /// Z-order for stacking (higher = on top).
    pub z_order: u16,
    /// Whether the window is visible.
    pub visible: bool,
    /// Whether the window can receive focus.
    pub focusable: bool,
    /// Buffer displayed in this window (if any).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub buffer_id: Option<BufferId>,
}

/// Complete layout state for the screen.
///
/// Contains all windows with their positions, the focused window,
/// and screen dimensions.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct WireLayoutInfo {
    /// Screen dimensions.
    #[serde(default)]
    pub screen: WireRect,
    /// All windows with their placements.
    #[serde(default)]
    pub windows: Vec<WireWindowPlacement>,
    /// Currently focused window (if any).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub focused_window: Option<WireWindowId>,
    /// Currently active layer (if any).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_layer: Option<WireLayerId>,
    /// Total number of windows.
    #[serde(default)]
    pub window_count: usize,
}

impl WireLayoutInfo {
    /// Create a single-window layout covering the full screen.
    #[must_use]
    pub fn single_window(width: u16, height: u16, buffer_id: Option<BufferId>) -> Self {
        let window = WireWindowPlacement {
            window_id: WireWindowId(0),
            layer_id: WireLayerId(0),
            zone: WireZone::Tiled,
            bounds: WireRect::full_screen(width, height),
            z_order: 0,
            visible: true,
            focusable: true,
            buffer_id,
        };
        Self {
            screen: WireRect::full_screen(width, height),
            windows: vec![window],
            focused_window: Some(WireWindowId(0)),
            active_layer: Some(WireLayerId(0)),
            window_count: 1,
        }
    }

    /// Check if this is a multi-window layout.
    #[must_use]
    pub const fn is_multi_window(&self) -> bool {
        self.windows.len() > 1
    }

    /// Get the focused window placement.
    #[must_use]
    pub fn focused(&self) -> Option<&WireWindowPlacement> {
        self.focused_window
            .and_then(|id| self.windows.iter().find(|w| w.window_id == id))
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
    pub active_window_id: Option<WireWindowId>,
    /// Total number of windows.
    pub window_count: usize,
}

/// Window information snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowInfo {
    /// Window ID.
    pub id: WireWindowId,
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
#[path = "types_tests.rs"]
mod tests;
