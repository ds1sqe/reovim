//! Edit operations for undo/redo support.
//!
//! This module defines atomic edit operations that can be recorded
//! and inverted for undo/redo functionality.

use super::Position;

/// A single atomic edit operation.
///
/// Edits are self-contained and can be inverted for undo/redo.
/// Each edit records the position where it occurred and the text involved.
///
/// # Undo/Redo
///
/// Use [`Edit::inverse`] to get the operation that undoes this edit:
/// - `Insert` becomes `Delete`
/// - `Delete` becomes `Insert`
///
/// # Example
///
/// ```
/// use reovim_kernel::mm::{Edit, Position};
///
/// let insert = Edit::insert(Position::new(0, 5), "Hello");
/// assert!(insert.is_insert());
/// assert_eq!(insert.text(), "Hello");
///
/// // Get the inverse for undo
/// let undo = insert.inverse();
/// assert!(undo.is_delete());
/// assert_eq!(undo.position(), Position::new(0, 5));
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Edit {
    /// Text was inserted at a position.
    Insert {
        /// Position where text was inserted.
        position: Position,
        /// The inserted text.
        text: String,
    },
    /// Text was deleted at a position.
    Delete {
        /// Position where deletion started.
        position: Position,
        /// The deleted text.
        text: String,
    },
}

impl Edit {
    /// Create an insert edit.
    #[must_use]
    pub fn insert(position: Position, text: impl Into<String>) -> Self {
        Self::Insert {
            position,
            text: text.into(),
        }
    }

    /// Create a delete edit.
    #[must_use]
    pub fn delete(position: Position, text: impl Into<String>) -> Self {
        Self::Delete {
            position,
            text: text.into(),
        }
    }

    /// Get the inverse of this edit (for undo).
    ///
    /// Insert becomes Delete and vice versa. The position and text
    /// are preserved.
    #[must_use]
    pub fn inverse(&self) -> Self {
        match self {
            Self::Insert { position, text } => Self::Delete {
                position: *position,
                text: text.clone(),
            },
            Self::Delete { position, text } => Self::Insert {
                position: *position,
                text: text.clone(),
            },
        }
    }

    /// Get the position where this edit occurred.
    #[must_use]
    pub const fn position(&self) -> Position {
        match self {
            Self::Insert { position, .. } | Self::Delete { position, .. } => *position,
        }
    }

    /// Get the text involved in this edit.
    #[must_use]
    pub fn text(&self) -> &str {
        match self {
            Self::Insert { text, .. } | Self::Delete { text, .. } => text,
        }
    }

    /// Check if this edit is an insertion.
    #[must_use]
    pub const fn is_insert(&self) -> bool {
        matches!(self, Self::Insert { .. })
    }

    /// Check if this edit is a deletion.
    #[must_use]
    pub const fn is_delete(&self) -> bool {
        matches!(self, Self::Delete { .. })
    }

    /// Check if this edit has no effect (empty text).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.text().is_empty()
    }
}
