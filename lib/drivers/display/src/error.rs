//! Display driver error types.

use std::fmt;

/// Errors that can occur during display operations.
#[derive(Debug)]
pub enum DisplayError {
    /// Driver not initialized.
    NotInitialized,
    /// Invalid terminal size.
    InvalidSize { width: u16, height: u16 },
    /// Render operation failed.
    RenderFailed(String),
    /// Window not found.
    WindowNotFound(usize),
    /// I/O error.
    Io(std::io::Error),
    /// Compositor error.
    CompositorError(String),
    /// Cursor position out of bounds.
    CursorOutOfBounds { x: u16, y: u16 },
}

impl fmt::Display for DisplayError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotInitialized => write!(f, "display driver not initialized"),
            Self::InvalidSize { width, height } => {
                write!(f, "invalid terminal size: {width}x{height}")
            }
            Self::RenderFailed(msg) => write!(f, "render failed: {msg}"),
            Self::WindowNotFound(id) => write!(f, "window not found: {id}"),
            Self::Io(err) => write!(f, "I/O error: {err}"),
            Self::CompositorError(msg) => write!(f, "compositor error: {msg}"),
            Self::CursorOutOfBounds { x, y } => {
                write!(f, "cursor position out of bounds: ({x}, {y})")
            }
        }
    }
}

impl std::error::Error for DisplayError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(err) => Some(err),
            _ => None,
        }
    }
}

impl From<std::io::Error> for DisplayError {
    fn from(err: std::io::Error) -> Self {
        Self::Io(err)
    }
}
