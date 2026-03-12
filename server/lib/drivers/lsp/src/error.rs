//! Error types for LSP operations.

use std::fmt;

/// Error type for LSP operations.
#[derive(Debug)]
pub enum LspError {
    /// Failed to spawn the server process.
    SpawnFailed(String),
    /// Transport/communication error.
    TransportError(String),
    /// Server returned an error response.
    ServerError {
        /// LSP error code.
        code: i32,
        /// Error message from server.
        message: String,
    },
    /// Request timed out.
    Timeout,
    /// Communication channel closed (server died).
    ChannelClosed,
    /// Serialization/deserialization error.
    Serialization(String),
    /// Server not initialized.
    NotInitialized,
    /// Server process not running.
    ServerNotRunning,
    /// Invalid or unexpected response.
    InvalidResponse(String),
}

impl fmt::Display for LspError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SpawnFailed(e) => write!(f, "Failed to spawn LSP server: {e}"),
            Self::TransportError(e) => write!(f, "LSP transport error: {e}"),
            Self::ServerError { code, message } => {
                write!(f, "LSP server error {code}: {message}")
            }
            Self::Timeout => write!(f, "LSP request timed out"),
            Self::ChannelClosed => write!(f, "LSP channel closed"),
            Self::Serialization(e) => write!(f, "LSP serialization error: {e}"),
            Self::NotInitialized => write!(f, "LSP server not initialized"),
            Self::ServerNotRunning => write!(f, "LSP server not running"),
            Self::InvalidResponse(e) => write!(f, "Invalid LSP response: {e}"),
        }
    }
}

impl std::error::Error for LspError {}

impl From<crate::transport::TransportError> for LspError {
    fn from(e: crate::transport::TransportError) -> Self {
        Self::TransportError(e.to_string())
    }
}

impl From<serde_json::Error> for LspError {
    fn from(e: serde_json::Error) -> Self {
        Self::Serialization(e.to_string())
    }
}

// Re-export ModuleError for consistency with other drivers
pub use reovim_kernel::api::v1::ModuleError;

#[cfg(test)]
#[path = "error_tests.rs"]
mod tests;
