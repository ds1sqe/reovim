//! Error types for event loop operations.

/// Error type for event loop operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EventLoopError {
    /// Error reading input.
    InputError(String),
    /// Error during rendering.
    RenderError(String),
}

impl std::fmt::Display for EventLoopError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InputError(msg) => write!(f, "input error: {msg}"),
            Self::RenderError(msg) => write!(f, "render error: {msg}"),
        }
    }
}

impl std::error::Error for EventLoopError {}
