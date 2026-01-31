//! Structured snapshot types for visual state
//!
//! These types provide a complete representation of the screen state
//! that can be serialized to JSON for external tools and AI assistants.

use serde::{Deserialize, Serialize};

use crate::rpc::state::CellSnapshot;

/// Complete visual snapshot of the screen
///
/// This provides everything needed to understand the current visual state:
/// - Dimensions and cell grid with styles
/// - Cursor position and context
/// - Layer visibility and bounds
/// - Plain text for quick inspection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisualSnapshot {
    /// Screen width in columns
    pub width: u16,
    /// Screen height in rows
    pub height: u16,
    /// Cell grid (row-major: cells[y][x])
    pub cells: Vec<Vec<CellSnapshot>>,
    /// Cursor position and context
    pub cursor: Option<CursorInfo>,
    /// Active layers with their bounds and visibility
    pub layers: Vec<LayerInfo>,
    /// Plain text representation (characters only, newline-separated rows)
    pub plain_text: String,
}

/// Cursor position with context information
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CursorInfo {
    /// Column (0-indexed)
    pub x: u16,
    /// Row (0-indexed)
    pub y: u16,
    /// Which layer the cursor belongs to (e.g., "editor", "telescope")
    pub layer: String,
}

/// Information about a visual layer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayerInfo {
    /// Layer name (e.g., "editor", "telescope", "which\_key")
    pub name: String,
    /// Z-order for rendering (higher = drawn on top)
    pub z_order: u8,
    /// Whether the layer is currently visible
    pub visible: bool,
    /// Layer bounds on screen
    pub bounds: BoundsInfo,
}

/// Rectangular bounds on screen
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoundsInfo {
    /// Left edge (0-indexed column)
    pub x: u16,
    /// Top edge (0-indexed row)
    pub y: u16,
    /// Width in columns
    pub width: u16,
    /// Height in rows
    pub height: u16,
}

impl BoundsInfo {
    /// Create new bounds
    #[must_use]
    pub const fn new(x: u16, y: u16, width: u16, height: u16) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    /// Check if a point is within these bounds
    #[must_use]
    pub const fn contains(&self, px: u16, py: u16) -> bool {
        px >= self.x && px < self.x + self.width && py >= self.y && py < self.y + self.height
    }
}

impl VisualSnapshot {
    /// Create an empty snapshot with given dimensions
    #[must_use]
    pub const fn new(width: u16, height: u16) -> Self {
        Self {
            width,
            height,
            cells: Vec::new(),
            cursor: None,
            layers: Vec::new(),
            plain_text: String::new(),
        }
    }

    /// Get the character at a specific position
    #[must_use]
    pub fn char_at(&self, x: u16, y: u16) -> Option<char> {
        self.cells
            .get(y as usize)
            .and_then(|row| row.get(x as usize))
            .map(|cell| cell.char)
    }

    /// Get a row as a string
    #[must_use]
    pub fn row_to_string(&self, y: u16) -> Option<String> {
        self.cells
            .get(y as usize)
            .map(|row| row.iter().map(|c| c.char).collect())
    }

    /// Get text from a rectangular region
    #[must_use]
    pub fn region_to_string(&self, x: u16, y: u16, width: u16, height: u16) -> String {
        let mut result = String::new();
        for row_idx in y..y.saturating_add(height) {
            if let Some(row) = self.cells.get(row_idx as usize) {
                for col_idx in x..x.saturating_add(width) {
                    if let Some(cell) = row.get(col_idx as usize) {
                        result.push(cell.char);
                    }
                }
            }
            if row_idx < y.saturating_add(height).saturating_sub(1) {
                result.push('\n');
            }
        }
        result
    }

    /// Check if a layer is visible by name
    #[must_use]
    pub fn is_layer_visible(&self, name: &str) -> bool {
        self.layers.iter().any(|l| l.name == name && l.visible)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bounds_contains() {
        let bounds = BoundsInfo::new(10, 5, 20, 10);
        assert!(bounds.contains(10, 5)); // top-left corner
        assert!(bounds.contains(29, 14)); // bottom-right corner
        assert!(!bounds.contains(9, 5)); // just outside left
        assert!(!bounds.contains(30, 5)); // just outside right
    }

    #[test]
    fn test_visual_snapshot_char_at() {
        let mut snap = VisualSnapshot::new(10, 5);
        snap.cells = vec![vec![CellSnapshot::new('H'), CellSnapshot::new('i')]];
        assert_eq!(snap.char_at(0, 0), Some('H'));
        assert_eq!(snap.char_at(1, 0), Some('i'));
        assert_eq!(snap.char_at(2, 0), None);
    }
}
