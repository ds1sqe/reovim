//! Anchor types for overlay positioning.
//!
//! Anchors define where overlays should be positioned relative to
//! buffer content, cursor, or screen coordinates. The client is
//! responsible for resolving anchors to screen positions.

use serde::{Deserialize, Serialize};

/// Anchor point for positioning overlays.
///
/// Overlays can be anchored to various reference points:
/// - Buffer positions (line/column in document)
/// - Cursor position (follows cursor movement)
/// - Screen coordinates (fixed position on screen)
/// - Relative positions (below another overlay)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "wasm", derive(tsify_next::Tsify))]
#[cfg_attr(feature = "wasm", tsify(into_wasm_abi, from_wasm_abi))]
pub enum Anchor {
    /// Anchor to a specific buffer position.
    ///
    /// The client resolves this to screen coordinates based on
    /// the viewport scroll position and buffer layout.
    Buffer {
        /// Buffer identifier.
        buffer_id: u64,
        /// 0-indexed line number.
        line: u32,
        /// 0-indexed column (byte offset within line).
        col: u32,
    },

    /// Anchor to the current cursor position.
    ///
    /// The overlay follows cursor movement automatically.
    Cursor,

    /// Anchor to fixed screen coordinates.
    ///
    /// Values are normalized (0.0 to 1.0) relative to screen size.
    /// This allows overlays to maintain relative position across resizes.
    Screen {
        /// Horizontal position (0.0 = left, 1.0 = right).
        x: f32,
        /// Vertical position (0.0 = top, 1.0 = bottom).
        y: f32,
    },

    /// Center the overlay on screen.
    ///
    /// Commonly used for modal dialogs and command palettes.
    Center,

    /// Position below another overlay.
    ///
    /// Used for chained UI elements like nested menus.
    Below(String),
}

impl Anchor {
    /// Create a buffer anchor.
    #[must_use]
    pub const fn buffer(buffer_id: u64, line: u32, col: u32) -> Self {
        Self::Buffer {
            buffer_id,
            line,
            col,
        }
    }

    /// Create a screen anchor with normalized coordinates.
    #[must_use]
    pub const fn screen(x: f32, y: f32) -> Self {
        Self::Screen { x, y }
    }

    /// Create a "below" anchor relative to another overlay.
    #[must_use]
    pub fn below(overlay_id: impl Into<String>) -> Self {
        Self::Below(overlay_id.into())
    }

    /// Check if this anchor tracks the cursor.
    #[must_use]
    pub const fn is_cursor_relative(&self) -> bool {
        matches!(self, Self::Cursor)
    }

    /// Check if this anchor is fixed on screen.
    #[must_use]
    pub const fn is_screen_fixed(&self) -> bool {
        matches!(self, Self::Screen { .. } | Self::Center)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_anchor_buffer() {
        let anchor = Anchor::buffer(1, 10, 5);
        match anchor {
            Anchor::Buffer {
                buffer_id,
                line,
                col,
            } => {
                assert_eq!(buffer_id, 1);
                assert_eq!(line, 10);
                assert_eq!(col, 5);
            }
            _ => panic!("Expected Buffer anchor"),
        }
    }

    #[test]
    fn test_anchor_cursor() {
        let anchor = Anchor::Cursor;
        assert!(anchor.is_cursor_relative());
        assert!(!anchor.is_screen_fixed());
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_anchor_screen() {
        let anchor = Anchor::screen(0.5, 0.5);
        assert!(anchor.is_screen_fixed());
        assert!(!anchor.is_cursor_relative());
        match anchor {
            Anchor::Screen { x, y } => {
                assert!((x - 0.5).abs() < f32::EPSILON);
                assert!((y - 0.5).abs() < f32::EPSILON);
            }
            _ => panic!("Expected Screen anchor"),
        }
    }

    #[test]
    fn test_anchor_center() {
        let anchor = Anchor::Center;
        assert!(anchor.is_screen_fixed());
        assert!(!anchor.is_cursor_relative());
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_anchor_below() {
        let anchor = Anchor::below("completion");
        match anchor {
            Anchor::Below(id) => assert_eq!(id, "completion"),
            _ => panic!("Expected Below anchor"),
        }
    }
}
