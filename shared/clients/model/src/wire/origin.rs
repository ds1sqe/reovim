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
#[path = "origin_tests.rs"]
mod tests;
