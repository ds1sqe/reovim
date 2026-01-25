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
/// Convert to/from `reovim_kernel::api::v1::WindowId` (the canonical type) at
/// RPC boundaries using the `From` implementations.
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

impl From<reovim_kernel::api::v1::WindowId> for WireWindowId {
    fn from(id: reovim_kernel::api::v1::WindowId) -> Self {
        Self(id.as_usize())
    }
}

impl From<WireWindowId> for reovim_kernel::api::v1::WindowId {
    fn from(id: WireWindowId) -> Self {
        Self::from_raw(id.0)
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

    #[test]
    fn test_wire_window_id_roundtrip() {
        use reovim_kernel::api::v1::WindowId;

        // Kernel → Wire → Kernel
        let kernel_id = WindowId::from_raw(42);
        let wire: WireWindowId = kernel_id.into();
        let back: WindowId = wire.into();
        assert_eq!(kernel_id, back);

        // Verify transparent serialization (just "42", not {"0": 42})
        let json = serde_json::to_string(&wire).unwrap();
        assert_eq!(json, "42");

        // Verify deserialization
        let parsed: WireWindowId = serde_json::from_str("42").unwrap();
        assert_eq!(parsed.0, 42);
    }

    // Layout types tests (#444)

    #[test]
    fn test_wire_layer_id_serialization() {
        let layer_id = WireLayerId(5);
        let json = serde_json::to_string(&layer_id).unwrap();
        assert_eq!(json, "5");

        let parsed: WireLayerId = serde_json::from_str("5").unwrap();
        assert_eq!(parsed.0, 5);
    }

    #[test]
    fn test_wire_rect_serialization() {
        let rect = WireRect::new(10, 20, 80, 24);
        let json = serde_json::to_string(&rect).unwrap();
        assert!(json.contains("\"x\":10"));
        assert!(json.contains("\"y\":20"));
        assert!(json.contains("\"width\":80"));
        assert!(json.contains("\"height\":24"));

        let parsed: WireRect = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, rect);
    }

    #[test]
    fn test_wire_rect_full_screen() {
        let rect = WireRect::full_screen(120, 40);
        assert_eq!(rect.x, 0);
        assert_eq!(rect.y, 0);
        assert_eq!(rect.width, 120);
        assert_eq!(rect.height, 40);
    }

    #[test]
    fn test_wire_zone_serialization() {
        assert_eq!(serde_json::to_string(&WireZone::Tiled).unwrap(), "\"tiled\"");
        assert_eq!(serde_json::to_string(&WireZone::Float).unwrap(), "\"float\"");
        assert_eq!(serde_json::to_string(&WireZone::Overlay).unwrap(), "\"overlay\"");
    }

    #[test]
    fn test_wire_split_direction_serialization() {
        assert_eq!(
            serde_json::to_string(&WireSplitDirection::Horizontal).unwrap(),
            "\"horizontal\""
        );
        assert_eq!(serde_json::to_string(&WireSplitDirection::Vertical).unwrap(), "\"vertical\"");
    }

    #[test]
    fn test_wire_layout_change_kind_split() {
        let kind = WireLayoutChangeKind::Split {
            new_window: WireWindowId(2),
            direction: WireSplitDirection::Vertical,
        };
        let json = serde_json::to_string(&kind).unwrap();
        assert!(json.contains("\"type\":\"split\""));
        assert!(json.contains("\"new_window\":2"));
        assert!(json.contains("\"direction\":\"vertical\""));
    }

    #[test]
    fn test_wire_layout_change_kind_close() {
        let kind = WireLayoutChangeKind::Close {
            closed_window: WireWindowId(1),
            new_focus: Some(WireWindowId(0)),
        };
        let json = serde_json::to_string(&kind).unwrap();
        assert!(json.contains("\"type\":\"close\""));
        assert!(json.contains("\"closed_window\":1"));
        assert!(json.contains("\"new_focus\":0"));
    }

    #[test]
    fn test_wire_layout_change_kind_focus() {
        let kind = WireLayoutChangeKind::Focus {
            from: Some(WireWindowId(0)),
            to: WireWindowId(1),
        };
        let json = serde_json::to_string(&kind).unwrap();
        assert!(json.contains("\"type\":\"focus\""));
        assert!(json.contains("\"from\":0"));
        assert!(json.contains("\"to\":1"));
    }

    #[test]
    fn test_wire_layout_change_kind_resize() {
        let kind = WireLayoutChangeKind::Resize {
            window: WireWindowId(1),
        };
        let json = serde_json::to_string(&kind).unwrap();
        assert!(json.contains("\"type\":\"resize\""));
        assert!(json.contains("\"window\":1"));
    }

    #[test]
    fn test_wire_layout_change_kind_equalize() {
        let kind = WireLayoutChangeKind::Equalize;
        let json = serde_json::to_string(&kind).unwrap();
        assert!(json.contains("\"type\":\"equalize\""));
    }

    #[test]
    fn test_wire_window_placement_serialization() {
        let placement = WireWindowPlacement {
            window_id: WireWindowId(1),
            layer_id: WireLayerId(0),
            zone: WireZone::Tiled,
            bounds: WireRect::new(0, 0, 40, 24),
            z_order: 100,
            visible: true,
            focusable: true,
            buffer_id: Some(BufferId(0)),
        };
        let json = serde_json::to_string(&placement).unwrap();
        assert!(json.contains("\"window_id\":1"));
        assert!(json.contains("\"layer_id\":0"));
        assert!(json.contains("\"zone\":\"tiled\""));
        assert!(json.contains("\"z_order\":100"));
        assert!(json.contains("\"visible\":true"));
        assert!(json.contains("\"buffer_id\":0"));

        let parsed: WireWindowPlacement = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, placement);
    }

    #[test]
    fn test_wire_window_placement_no_buffer() {
        let placement = WireWindowPlacement {
            window_id: WireWindowId(1),
            layer_id: WireLayerId(0),
            zone: WireZone::Float,
            bounds: WireRect::new(10, 10, 30, 20),
            z_order: 200,
            visible: true,
            focusable: true,
            buffer_id: None,
        };
        let json = serde_json::to_string(&placement).unwrap();
        // buffer_id should be skipped when None
        assert!(!json.contains("\"buffer_id\""));
    }

    #[test]
    fn test_wire_layout_info_empty() {
        let layout = WireLayoutInfo::default();
        assert_eq!(layout.window_count, 0);
        assert!(layout.windows.is_empty());
        assert!(layout.focused_window.is_none());
    }

    #[test]
    fn test_wire_layout_info_single_window() {
        let layout = WireLayoutInfo::single_window(80, 24, Some(BufferId(0)));
        assert_eq!(layout.window_count, 1);
        assert_eq!(layout.windows.len(), 1);
        assert!(layout.focused_window.is_some());
        assert!(!layout.is_multi_window());
        assert!(layout.focused().is_some());
    }

    #[test]
    fn test_wire_layout_info_multi_window() {
        let layout = WireLayoutInfo {
            screen: WireRect::full_screen(80, 24),
            windows: vec![
                WireWindowPlacement {
                    window_id: WireWindowId(0),
                    layer_id: WireLayerId(0),
                    zone: WireZone::Tiled,
                    bounds: WireRect::new(0, 0, 40, 24),
                    z_order: 100,
                    visible: true,
                    focusable: true,
                    buffer_id: Some(BufferId(0)),
                },
                WireWindowPlacement {
                    window_id: WireWindowId(1),
                    layer_id: WireLayerId(0),
                    zone: WireZone::Tiled,
                    bounds: WireRect::new(40, 0, 40, 24),
                    z_order: 100,
                    visible: true,
                    focusable: true,
                    buffer_id: Some(BufferId(0)),
                },
            ],
            focused_window: Some(WireWindowId(1)),
            active_layer: Some(WireLayerId(0)),
            window_count: 2,
        };
        assert!(layout.is_multi_window());
        let focused = layout.focused().unwrap();
        assert_eq!(focused.window_id, WireWindowId(1));
    }

    #[test]
    fn test_wire_layout_info_serialization_roundtrip() {
        let layout = WireLayoutInfo::single_window(120, 40, Some(BufferId(1)));
        let json = serde_json::to_string(&layout).unwrap();
        let parsed: WireLayoutInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, layout);
    }
}
