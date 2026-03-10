//! Semantic origin metadata for extension data.
//!
//! `SemanticOrigin` tells clients WHAT data relates to — not WHERE to render it.
//! The server provides informational context about the data's origin; each client
//! independently decides how and where to render based on its own constraints
//! (terminal grid, browser DOM, native GUI).

use serde::{Deserialize, Serialize};

/// Informational origin — tells clients WHAT this data relates to.
///
/// This is NOT a positioning directive. Clients decide rendering independently.
///
/// # Examples
///
/// - Hover info relates to the symbol at line 5, col 12 → `BufferPosition`
/// - Diagnostics cover lines 5–7 → `BufferRange`
/// - Buffer-wide diagnostics summary → `Buffer`
/// - Notifications, which-key hints → `Session`
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "wasm", derive(tsify_next::Tsify))]
#[cfg_attr(feature = "wasm", tsify(into_wasm_abi, from_wasm_abi))]
pub enum SemanticOrigin {
    /// Data relates to a specific buffer position.
    BufferPosition {
        /// Buffer identifier.
        buffer_id: u64,
        /// 0-indexed line number.
        line: u32,
        /// 0-indexed column (byte offset within line).
        col: u32,
    },

    /// Data relates to a buffer range.
    BufferRange {
        /// Buffer identifier.
        buffer_id: u64,
        /// Start line (0-indexed).
        start_line: u32,
        /// Start column (0-indexed).
        start_col: u32,
        /// End line (0-indexed).
        end_line: u32,
        /// End column (0-indexed).
        end_col: u32,
    },

    /// Data relates to a buffer (no specific position).
    Buffer {
        /// Buffer identifier.
        buffer_id: u64,
    },

    /// Data relates to the session globally.
    Session,
}

impl SemanticOrigin {
    /// Create a buffer-position origin.
    #[must_use]
    pub const fn buffer_position(buffer_id: u64, line: u32, col: u32) -> Self {
        Self::BufferPosition {
            buffer_id,
            line,
            col,
        }
    }

    /// Create a buffer-range origin.
    #[must_use]
    pub const fn buffer_range(
        buffer_id: u64,
        start_line: u32,
        start_col: u32,
        end_line: u32,
        end_col: u32,
    ) -> Self {
        Self::BufferRange {
            buffer_id,
            start_line,
            start_col,
            end_line,
            end_col,
        }
    }

    /// Create a buffer-level origin (no specific position).
    #[must_use]
    pub const fn buffer(buffer_id: u64) -> Self {
        Self::Buffer { buffer_id }
    }

    /// Create a session-level origin.
    #[must_use]
    pub const fn session() -> Self {
        Self::Session
    }

    /// Return the buffer ID if this origin relates to a specific buffer.
    #[must_use]
    pub const fn buffer_id(&self) -> Option<u64> {
        match self {
            Self::BufferPosition { buffer_id, .. }
            | Self::BufferRange { buffer_id, .. }
            | Self::Buffer { buffer_id } => Some(*buffer_id),
            Self::Session => None,
        }
    }

    /// Check if this origin is session-level (no buffer association).
    #[must_use]
    pub const fn is_session(&self) -> bool {
        matches!(self, Self::Session)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn buffer_position_constructor() {
        let origin = SemanticOrigin::buffer_position(1, 5, 12);
        assert_eq!(
            origin,
            SemanticOrigin::BufferPosition {
                buffer_id: 1,
                line: 5,
                col: 12,
            }
        );
    }

    #[test]
    fn buffer_range_constructor() {
        let origin = SemanticOrigin::buffer_range(2, 5, 0, 7, 15);
        assert_eq!(
            origin,
            SemanticOrigin::BufferRange {
                buffer_id: 2,
                start_line: 5,
                start_col: 0,
                end_line: 7,
                end_col: 15,
            }
        );
    }

    #[test]
    fn buffer_constructor() {
        let origin = SemanticOrigin::buffer(3);
        assert_eq!(origin, SemanticOrigin::Buffer { buffer_id: 3 });
    }

    #[test]
    fn session_constructor() {
        let origin = SemanticOrigin::session();
        assert_eq!(origin, SemanticOrigin::Session);
    }

    #[test]
    fn buffer_id_returns_id_for_position() {
        assert_eq!(SemanticOrigin::buffer_position(1, 0, 0).buffer_id(), Some(1));
    }

    #[test]
    fn buffer_id_returns_id_for_range() {
        assert_eq!(SemanticOrigin::buffer_range(2, 0, 0, 1, 0).buffer_id(), Some(2));
    }

    #[test]
    fn buffer_id_returns_id_for_buffer() {
        assert_eq!(SemanticOrigin::buffer(3).buffer_id(), Some(3));
    }

    #[test]
    fn buffer_id_returns_none_for_session() {
        assert_eq!(SemanticOrigin::session().buffer_id(), None);
    }

    #[test]
    fn is_session_true() {
        assert!(SemanticOrigin::session().is_session());
    }

    #[test]
    fn is_session_false_for_buffer_position() {
        assert!(!SemanticOrigin::buffer_position(1, 0, 0).is_session());
    }

    #[test]
    fn is_session_false_for_buffer_range() {
        assert!(!SemanticOrigin::buffer_range(1, 0, 0, 1, 0).is_session());
    }

    #[test]
    fn is_session_false_for_buffer() {
        assert!(!SemanticOrigin::buffer(1).is_session());
    }

    #[test]
    fn clone_and_debug() {
        let origin = SemanticOrigin::buffer_position(1, 5, 12);
        let cloned = origin.clone();
        assert_eq!(origin, cloned);
        let debug = format!("{origin:?}");
        assert!(debug.contains("BufferPosition"));
    }

    #[test]
    fn serde_roundtrip_buffer_position() {
        let origin = SemanticOrigin::buffer_position(1, 5, 12);
        let json = serde_json::to_string(&origin).unwrap();
        let deserialized: SemanticOrigin = serde_json::from_str(&json).unwrap();
        assert_eq!(origin, deserialized);
    }

    #[test]
    fn serde_roundtrip_buffer_range() {
        let origin = SemanticOrigin::buffer_range(2, 5, 0, 7, 15);
        let json = serde_json::to_string(&origin).unwrap();
        let deserialized: SemanticOrigin = serde_json::from_str(&json).unwrap();
        assert_eq!(origin, deserialized);
    }

    #[test]
    fn serde_roundtrip_buffer() {
        let origin = SemanticOrigin::buffer(3);
        let json = serde_json::to_string(&origin).unwrap();
        let deserialized: SemanticOrigin = serde_json::from_str(&json).unwrap();
        assert_eq!(origin, deserialized);
    }

    #[test]
    fn serde_roundtrip_session() {
        let origin = SemanticOrigin::session();
        let json = serde_json::to_string(&origin).unwrap();
        let deserialized: SemanticOrigin = serde_json::from_str(&json).unwrap();
        assert_eq!(origin, deserialized);
    }
}
