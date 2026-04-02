//! Window management and focus operations.
//!
//! This module provides the [`WindowApi`] trait for window manipulation.
//! Resolvers and commands use this to manage window layout and focus.
//!
//! # Design
//!
//! Following Unix philosophy: this trait does ONE thing well - window management.
//!
//! # Example
//!
//! ```ignore
//! use reovim_driver_session::api::WindowApi;
//!
//! fn split_window<S: WindowApi + BufferApi>(session: &mut S) {
//!     let buffer = session.active_buffer();
//!     let new_window = session.create_window(buffer);
//!     session.focus_window(new_window).ok();
//! }
//! ```

use reovim_kernel::api::v1::{BufferId, WindowId};
use reovim_types_text::Position;

use super::Selection;

/// Window management and focus operations.
///
/// Provides access to window layout, focus, and buffer association
/// for resolvers and commands.
pub trait WindowApi: Send {
    /// Get the active window ID.
    fn active_window(&self) -> Option<WindowId>;

    /// Get cursor position from the active window.
    ///
    /// Returns `None` if no active window exists.
    /// Note: This returns cursor for the current client's active window.
    fn cursor_position(&self) -> Option<Position>;

    /// Get the window count.
    fn window_count(&self) -> usize;

    /// Get the buffer displayed in a window.
    fn window_buffer(&self, window: WindowId) -> Option<BufferId>;

    /// Create a new window.
    ///
    /// If `buffer` is `None`, the window starts empty.
    fn create_window(&mut self, buffer: Option<BufferId>) -> WindowId;

    /// Close a window.
    ///
    /// # Errors
    ///
    /// Returns error if window doesn't exist or is the last window.
    fn close_window(&mut self, window: WindowId) -> Result<(), WindowError>;

    /// Focus a window.
    ///
    /// # Errors
    ///
    /// Returns error if window doesn't exist.
    fn focus_window(&mut self, window: WindowId) -> Result<(), WindowError>;

    /// Set the buffer displayed in a window.
    ///
    /// # Errors
    ///
    /// Returns error if window or buffer doesn't exist.
    fn set_window_buffer(&mut self, window: WindowId, buffer: BufferId) -> Result<(), WindowError>;

    /// Set or clear the selection on the active window.
    ///
    /// Pass `None` to clear any existing selection.
    /// Used by snippet navigation and visual mode for programmatic
    /// selection control through the trait interface.
    fn set_active_selection(&mut self, selection: Option<Selection>);

    /// Get the selection on the active window, if any.
    fn active_selection(&self) -> Option<&Selection>;
}

/// Errors from window operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WindowError {
    /// Window not found.
    NotFound(WindowId),
    /// Cannot close the last window.
    CannotCloseLastWindow,
    /// Buffer not found.
    BufferNotFound(BufferId),
}

impl std::fmt::Display for WindowError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFound(id) => write!(f, "window not found: {id:?}"),
            Self::CannotCloseLastWindow => write!(f, "cannot close last window"),
            Self::BufferNotFound(id) => write!(f, "buffer not found: {id:?}"),
        }
    }
}

impl std::error::Error for WindowError {}
#[cfg(test)]
#[path = "tests/window.rs"]
mod tests;
