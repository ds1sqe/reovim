//! Buffer content and lifecycle operations.
//!
//! This module provides the [`BufferApi`] trait for buffer manipulation.
//! Resolvers and commands use this to query and modify buffer content.
//!
//! # Design
//!
//! Following Unix philosophy: this trait does ONE thing well - **buffer content** management.
//!
//! **Note (#471):** Cursor and selection are per-window, not per-buffer.
//! Commands receive cursor position via `CommandContext::cursor_position()`,
//! not from this trait.
//!
//! # Example
//!
//! ```ignore
//! use reovim_driver_session::api::BufferApi;
//! use reovim_driver_command_types::CommandContext;
//!
//! fn delete_word<S: BufferApi>(session: &mut S, args: &CommandContext) {
//!     if let Some(buffer) = session.active_buffer() {
//!         // Cursor comes from CommandContext, not BufferApi
//!         if let Some(pos) = args.cursor_position() {
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
/// Provides access to buffer **content** for resolvers and commands.
///
/// **Note (#471):** Cursor and selection are NOT part of this trait.
/// They are per-window properties, passed via `CommandContext`.
pub trait BufferApi: Send {
    // === Queries ===

    /// Get the active buffer ID.
    fn active_buffer(&self) -> Option<BufferId>;

    /// Set the active buffer ID.
    ///
    /// Updates the session-level active buffer. Use after creating a new
    /// buffer that should become the current editing target.
    fn set_active_buffer(&mut self, id: Option<BufferId>);

    /// Get a line from a buffer.
    ///
    /// Returns `None` if the buffer doesn't exist or line is out of bounds.
    fn buffer_line(&self, buffer: BufferId, line: usize) -> Option<String>;

    /// Get the line count of a buffer.
    ///
    /// Returns `None` if the buffer doesn't exist.
    fn buffer_line_count(&self, buffer: BufferId) -> Option<usize>;

    /// Get the length of a line in characters.
    ///
    /// Returns `None` if the buffer doesn't exist or line is out of bounds.
    fn buffer_line_len(&self, buffer: BufferId, line: usize) -> Option<usize>;

    /// Extract text from a range in the buffer.
    ///
    /// Returns the text between start and end positions, including all lines
    /// in between. Returns `None` if the buffer doesn't exist.
    ///
    /// # Range Semantics
    ///
    /// The range is inclusive of `start` and exclusive of `end`, similar to
    /// Rust's `start..end` range syntax.
    fn buffer_text_range(&self, buffer: BufferId, start: Position, end: Position)
    -> Option<String>;

    /// Get full buffer content as a string.
    ///
    /// Returns `None` if the buffer doesn't exist.
    fn buffer_content(&self, buffer: BufferId) -> Option<String>;

    /// Get buffer's file path.
    ///
    /// Returns `None` if the buffer doesn't exist or has no associated file.
    fn buffer_file_path(&self, buffer: BufferId) -> Option<String>;

    /// Check if buffer has been modified since last save.
    ///
    /// Returns `None` if buffer doesn't exist.
    fn is_buffer_modified(&self, buffer: BufferId) -> Option<bool>;

    /// Set buffer's modified flag.
    ///
    /// Used to mark buffer as saved (false) or modified (true).
    fn set_buffer_modified(&mut self, buffer: BufferId, modified: bool);

    // === Content Mutations ===

    /// Insert text at a position.
    fn insert_text(&mut self, buffer: BufferId, pos: Position, text: &str);

    /// Delete a range.
    fn delete_range(&mut self, buffer: BufferId, start: Position, end: Position);

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
#[path = "tests/buffer.rs"]
mod tests;
