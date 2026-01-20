//! Buffer content and lifecycle operations.
//!
//! This module provides the [`BufferApi`] trait for buffer manipulation.
//! Resolvers and commands use this to query and modify buffer content.
//!
//! # Design
//!
//! Following Unix philosophy: this trait does ONE thing well - buffer management.
//!
//! # Example
//!
//! ```ignore
//! use reovim_driver_session::api::BufferApi;
//!
//! fn delete_word<S: BufferApi>(session: &mut S) {
//!     if let Some(buffer) = session.active_buffer() {
//!         if let Some(pos) = session.cursor_position(buffer) {
//!             // Calculate word end and delete
//!             let end = Position::new(pos.line, pos.column + 4);
//!             session.delete_range(buffer, pos, end);
//!         }
//!     }
//! }
//! ```

use reovim_kernel::api::v1::{BufferId, Position};

/// Buffer content and lifecycle operations.
///
/// Provides access to buffer content, cursor, and selection for
/// resolvers and commands.
pub trait BufferApi: Send {
    // === Queries ===

    /// Get the active buffer ID.
    fn active_buffer(&self) -> Option<BufferId>;

    /// Get a line from a buffer.
    ///
    /// Returns `None` if the buffer doesn't exist or line is out of bounds.
    fn buffer_line(&self, buffer: BufferId, line: usize) -> Option<String>;

    /// Get the line count of a buffer.
    ///
    /// Returns `None` if the buffer doesn't exist.
    fn buffer_line_count(&self, buffer: BufferId) -> Option<usize>;

    /// Get cursor position in a buffer (window cursor).
    ///
    /// Returns the cursor position from the window displaying this buffer.
    /// Returns `None` if no window displays this buffer.
    ///
    /// **Note:** This is the *window* cursor, not the buffer's internal position.
    /// For the buffer's intrinsic position, use [`buffer_position`](Self::buffer_position).
    fn cursor_position(&self, buffer: BufferId) -> Option<Position>;

    /// Get buffer's internal position.
    ///
    /// Returns the buffer's intrinsic cursor position (stored in kernel).
    /// This may differ from the window cursor position.
    ///
    /// Returns `None` if the buffer doesn't exist.
    fn buffer_position(&self, buffer: BufferId) -> Option<Position>;

    /// Set buffer's internal position.
    ///
    /// Updates the buffer's intrinsic cursor position (stored in kernel).
    /// Does not affect the window cursor position.
    fn set_buffer_position(&mut self, buffer: BufferId, pos: Position);

    /// Get the length of a line in characters.
    ///
    /// Returns `None` if the buffer doesn't exist or line is out of bounds.
    fn buffer_line_len(&self, buffer: BufferId, line: usize) -> Option<usize>;

    /// Get selection in a buffer.
    ///
    /// Returns `None` if no selection is active.
    fn selection(&self, buffer: BufferId) -> Option<Selection>;

    // === Content Mutations ===

    /// Insert text at a position.
    fn insert_text(&mut self, buffer: BufferId, pos: Position, text: &str);

    /// Delete a range.
    fn delete_range(&mut self, buffer: BufferId, start: Position, end: Position);

    /// Move cursor to a position.
    fn move_cursor(&mut self, buffer: BufferId, pos: Position);

    /// Set selection.
    fn set_selection(&mut self, buffer: BufferId, sel: Option<Selection>);

    // === Lifecycle ===

    /// Create a new buffer.
    ///
    /// Returns the ID of the newly created buffer.
    fn create_buffer(&mut self, name: Option<&str>, content: &str) -> BufferId;

    /// Delete a buffer.
    ///
    /// # Errors
    ///
    /// Returns error if buffer doesn't exist or is the last buffer.
    fn delete_buffer(&mut self, buffer: BufferId) -> Result<(), BufferError>;

    /// Rename a buffer.
    fn rename_buffer(&mut self, buffer: BufferId, new_name: &str);
}

/// Selection in a buffer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Selection {
    /// Start position of the selection.
    pub start: Position,
    /// End position of the selection.
    pub end: Position,
    /// Selection mode (character, line, block).
    pub mode: SelectionMode,
}

impl Selection {
    /// Create a new selection.
    #[must_use]
    pub const fn new(start: Position, end: Position, mode: SelectionMode) -> Self {
        Self { start, end, mode }
    }

    /// Create a character-mode selection.
    #[must_use]
    pub const fn character(start: Position, end: Position) -> Self {
        Self::new(start, end, SelectionMode::Character)
    }

    /// Create a line-mode selection.
    #[must_use]
    pub const fn line(start: Position, end: Position) -> Self {
        Self::new(start, end, SelectionMode::Line)
    }

    /// Create a block-mode selection.
    #[must_use]
    pub const fn block(start: Position, end: Position) -> Self {
        Self::new(start, end, SelectionMode::Block)
    }

    /// Check if the selection is linewise.
    #[must_use]
    pub const fn is_linewise(&self) -> bool {
        matches!(self.mode, SelectionMode::Line)
    }
}

/// Selection mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SelectionMode {
    /// Character-wise selection (v in vim).
    #[default]
    Character,
    /// Line-wise selection (V in vim).
    Line,
    /// Block/column selection (Ctrl-v in vim).
    Block,
}

/// Errors from buffer operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BufferError {
    /// Buffer not found.
    NotFound(BufferId),
    /// Cannot delete the last buffer.
    CannotDeleteLastBuffer,
}

impl std::fmt::Display for BufferError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFound(id) => write!(f, "buffer not found: {id:?}"),
            Self::CannotDeleteLastBuffer => write!(f, "cannot delete last buffer"),
        }
    }
}

impl std::error::Error for BufferError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_selection_modes() {
        let start = Position::new(0, 0);
        let end = Position::new(0, 5);

        let char_sel = Selection::character(start, end);
        assert!(!char_sel.is_linewise());
        assert_eq!(char_sel.mode, SelectionMode::Character);

        let line_sel = Selection::line(start, end);
        assert!(line_sel.is_linewise());
        assert_eq!(line_sel.mode, SelectionMode::Line);

        let block_sel = Selection::block(start, end);
        assert!(!block_sel.is_linewise());
        assert_eq!(block_sel.mode, SelectionMode::Block);
    }

    #[test]
    fn test_buffer_error_display() {
        let err = BufferError::CannotDeleteLastBuffer;
        assert_eq!(err.to_string(), "cannot delete last buffer");

        let id = BufferId::new();
        let err = BufferError::NotFound(id);
        assert!(err.to_string().contains("buffer not found"));
    }

    #[test]
    fn test_selection_mode_default() {
        let mode = SelectionMode::default();
        assert_eq!(mode, SelectionMode::Character);
    }
}
