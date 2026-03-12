//! Display driver traits.

use crate::{
    DisplayCapabilities, DisplayError, FrameBuffer, NavigateDirection, Rect, RenderCommand,
    SplitDirection, TerminalSize, WindowId, highlight::ColorMode,
};

/// Display driver for terminal operations.
///
/// Manages the terminal lifecycle, frame buffer, and rendering.
pub trait DisplayDriver: Send + Sync {
    /// Initialize the display driver.
    ///
    /// # Errors
    ///
    /// Returns `DisplayError` if initialization fails.
    fn init(&mut self) -> Result<(), DisplayError>;

    /// Shutdown the display driver.
    ///
    /// # Errors
    ///
    /// Returns `DisplayError` if shutdown fails.
    fn shutdown(&mut self) -> Result<(), DisplayError>;

    /// Check if initialized.
    fn is_initialized(&self) -> bool;

    /// Get mutable access to the frame buffer.
    fn frame_buffer_mut(&mut self) -> &mut FrameBuffer;

    /// Get read-only access to the frame buffer.
    fn frame_buffer(&self) -> &FrameBuffer;

    /// Render the frame buffer to terminal (diff-based).
    ///
    /// # Errors
    ///
    /// Returns `DisplayError::RenderFailed` if rendering fails.
    fn render(&mut self) -> Result<(), DisplayError>;

    /// Force full redraw (invalidate diff cache).
    fn invalidate(&mut self);

    /// Resize the display.
    ///
    /// # Errors
    ///
    /// Returns `DisplayError::InvalidSize` if the size is invalid.
    fn resize(&mut self, size: TerminalSize) -> Result<(), DisplayError>;

    /// Get current terminal size.
    fn size(&self) -> TerminalSize;

    /// Get terminal capabilities.
    fn capabilities(&self) -> &DisplayCapabilities;

    /// Get cursor position.
    fn cursor_position(&self) -> (u16, u16);

    /// Set cursor position.
    ///
    /// # Errors
    ///
    /// Returns `DisplayError::CursorOutOfBounds` if position is out of bounds.
    fn set_cursor_position(&mut self, x: u16, y: u16) -> Result<(), DisplayError>;

    /// Hide the cursor.
    ///
    /// # Errors
    ///
    /// Returns `DisplayError::Io` if the terminal operation fails.
    fn hide_cursor(&mut self) -> Result<(), DisplayError>;

    /// Show the cursor.
    ///
    /// # Errors
    ///
    /// Returns `DisplayError::Io` if the terminal operation fails.
    fn show_cursor(&mut self) -> Result<(), DisplayError>;

    /// Get current color mode.
    fn color_mode(&self) -> ColorMode;

    /// Set color mode (runtime switching).
    fn set_color_mode(&mut self, mode: ColorMode);

    /// Execute render commands directly.
    ///
    /// # Errors
    ///
    /// Returns `DisplayError::RenderFailed` if command execution fails.
    fn execute_commands(&mut self, commands: &[RenderCommand]) -> Result<(), DisplayError>;
}

/// Window manager for layout management.
///
/// Manages window splits, navigation, and layout operations.
pub trait WindowManager: Send + Sync {
    /// Get the active window ID.
    fn active_window(&self) -> Option<WindowId>;

    /// Set the active window.
    ///
    /// # Errors
    ///
    /// Returns `DisplayError::WindowNotFound` if the window doesn't exist.
    fn set_active_window(&mut self, id: WindowId) -> Result<(), DisplayError>;

    /// Create a new window.
    ///
    /// # Errors
    ///
    /// Returns `DisplayError` if window creation fails.
    fn create_window(&mut self) -> Result<WindowId, DisplayError>;

    /// Close a window.
    ///
    /// # Errors
    ///
    /// Returns `DisplayError::WindowNotFound` if the window doesn't exist.
    fn close_window(&mut self, id: WindowId) -> Result<(), DisplayError>;

    /// Split the active window.
    ///
    /// # Errors
    ///
    /// Returns `DisplayError` if split fails (e.g., no active window).
    fn split(&mut self, direction: SplitDirection) -> Result<WindowId, DisplayError>;

    /// Get window bounds.
    fn bounds(&self, id: WindowId) -> Option<Rect>;

    /// Navigate to adjacent window.
    fn navigate(&mut self, direction: NavigateDirection) -> Option<WindowId>;

    /// Get all window IDs.
    fn window_ids(&self) -> Vec<WindowId>;

    /// Get window count.
    fn window_count(&self) -> usize;

    /// Equalize window sizes.
    fn equalize(&mut self);

    /// Resize active window.
    fn resize_window(&mut self, direction: SplitDirection, delta: f32);

    /// Swap active window with neighbor.
    fn swap(&mut self, direction: NavigateDirection) -> bool;
}

#[cfg(test)]
#[path = "traits_tests.rs"]
mod tests;
