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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_display_not_initialized() {
        let err = DisplayError::NotInitialized;
        assert_eq!(format!("{err}"), "display driver not initialized");
    }

    #[test]
    fn test_display_invalid_size() {
        let err = DisplayError::InvalidSize {
            width: 0,
            height: 0,
        };
        assert_eq!(format!("{err}"), "invalid terminal size: 0x0");
    }

    #[test]
    fn test_display_render_failed() {
        let err = DisplayError::RenderFailed("test error".to_string());
        assert_eq!(format!("{err}"), "render failed: test error");
    }

    #[test]
    fn test_display_window_not_found() {
        let err = DisplayError::WindowNotFound(42);
        assert_eq!(format!("{err}"), "window not found: 42");
    }

    #[test]
    fn test_display_io_error() {
        let io_err = std::io::Error::new(std::io::ErrorKind::BrokenPipe, "pipe broke");
        let err = DisplayError::Io(io_err);
        let msg = format!("{err}");
        assert!(msg.starts_with("I/O error:"));
        assert!(msg.contains("pipe broke"));
    }

    #[test]
    fn test_display_compositor_error() {
        let err = DisplayError::CompositorError("layer conflict".to_string());
        assert_eq!(format!("{err}"), "compositor error: layer conflict");
    }

    #[test]
    fn test_display_cursor_out_of_bounds() {
        let err = DisplayError::CursorOutOfBounds { x: 100, y: 50 };
        assert_eq!(format!("{err}"), "cursor position out of bounds: (100, 50)");
    }

    #[test]
    fn test_error_source_io() {
        use std::error::Error;
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "not found");
        let err = DisplayError::Io(io_err);
        assert!(err.source().is_some());
    }

    #[test]
    fn test_error_source_non_io() {
        use std::error::Error;
        let err = DisplayError::NotInitialized;
        assert!(err.source().is_none());

        let err = DisplayError::RenderFailed("msg".to_string());
        assert!(err.source().is_none());

        let err = DisplayError::WindowNotFound(1);
        assert!(err.source().is_none());

        let err = DisplayError::CompositorError("msg".to_string());
        assert!(err.source().is_none());

        let err = DisplayError::CursorOutOfBounds { x: 0, y: 0 };
        assert!(err.source().is_none());

        let err = DisplayError::InvalidSize {
            width: 0,
            height: 0,
        };
        assert!(err.source().is_none());
    }

    #[test]
    fn test_from_io_error() {
        let io_err = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "denied");
        let err: DisplayError = io_err.into();
        assert!(matches!(err, DisplayError::Io(_)));
    }

    #[test]
    fn test_debug_impl() {
        let err = DisplayError::NotInitialized;
        let debug = format!("{err:?}");
        assert!(debug.contains("NotInitialized"));
    }
}
