//! Undo/redo history using change-based deltas

use crate::screen::Position;

/// A single atomic change that can be undone/redone
#[derive(Debug, Clone)]
pub enum Change {
    /// Text was inserted at position
    Insert { pos: Position, text: String },
    /// Text was deleted at position
    Delete { pos: Position, text: String },
    /// Multiple changes grouped as one undo unit
    Batch(Vec<Self>),
}

impl Change {
    /// Create the inverse of this change (for undo)
    #[must_use]
    pub fn inverse(&self) -> Self {
        match self {
            Self::Insert { pos, text } => Self::Delete {
                pos: *pos,
                text: text.clone(),
            },
            Self::Delete { pos, text } => Self::Insert {
                pos: *pos,
                text: text.clone(),
            },
            Self::Batch(changes) => {
                // Reverse the order and invert each change
                Self::Batch(changes.iter().rev().map(Self::inverse).collect())
            }
        }
    }
}

/// Manages undo/redo history
#[derive(Debug, Clone, Default)]
pub struct UndoHistory {
    /// Stack of changes that can be undone
    undo_stack: Vec<Change>,
    /// Stack of undone changes that can be redone
    redo_stack: Vec<Change>,
    /// Maximum number of undo levels
    max_size: usize,
}

impl UndoHistory {
    /// Create a new history with default max size
    #[must_use]
    pub const fn new() -> Self {
        Self {
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            max_size: 1000,
        }
    }

    /// Record a new change
    pub fn push(&mut self, change: Change) {
        // Clear redo stack when new changes are made
        self.redo_stack.clear();

        self.undo_stack.push(change);

        // Trim to max size if needed
        if self.undo_stack.len() > self.max_size {
            self.undo_stack.remove(0);
        }
    }

    /// Pop a change for undoing
    /// Returns the change to apply to reverse the action
    pub fn undo(&mut self) -> Option<Change> {
        let change = self.undo_stack.pop()?;
        let inverse = change.inverse();
        self.redo_stack.push(change);
        Some(inverse)
    }

    /// Pop a change for redoing
    /// Returns the change to apply to redo the action
    pub fn redo(&mut self) -> Option<Change> {
        let change = self.redo_stack.pop()?;
        self.undo_stack.push(change.clone());
        Some(change)
    }

    /// Check if undo is available
    #[must_use]
    pub const fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    /// Check if redo is available
    #[must_use]
    pub const fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    /// Clear all history
    pub fn clear(&mut self) {
        self.undo_stack.clear();
        self.redo_stack.clear();
    }
}
