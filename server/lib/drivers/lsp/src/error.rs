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
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = LspError::NotInitialized;
        assert_eq!(format!("{err}"), "LSP server not initialized");

        let err = LspError::ServerError {
            code: -32600,
            message: "Invalid request".to_string(),
        };
        let display = format!("{err}");
        assert!(display.contains("-32600"));
        assert!(display.contains("Invalid request"));
    }

    #[test]
    fn test_spawn_failed_display() {
        let err = LspError::SpawnFailed("command not found".to_string());
        assert!(format!("{err}").contains("command not found"));
    }

    #[test]
    fn test_transport_error_display() {
        let err = LspError::TransportError("connection reset".to_string());
        assert!(format!("{err}").contains("connection reset"));
    }

    #[test]
    fn test_timeout_display() {
        let err = LspError::Timeout;
        assert_eq!(format!("{err}"), "LSP request timed out");
    }

    #[test]
    fn test_channel_closed_display() {
        let err = LspError::ChannelClosed;
        assert_eq!(format!("{err}"), "LSP channel closed");
    }

    #[test]
    fn test_serialization_display() {
        let err = LspError::Serialization("invalid JSON".to_string());
        assert!(format!("{err}").contains("invalid JSON"));
    }

    #[test]
    fn test_server_not_running_display() {
        let err = LspError::ServerNotRunning;
        assert_eq!(format!("{err}"), "LSP server not running");
    }

    #[test]
    fn test_invalid_response_display() {
        let err = LspError::InvalidResponse("unexpected field".to_string());
        assert!(format!("{err}").contains("unexpected field"));
    }

    #[test]
    fn test_error_is_error_trait() {
        fn assert_error<E: std::error::Error>() {}
        assert_error::<LspError>();
    }

    #[test]
    fn test_from_transport_error() {
        let transport_err = crate::transport::TransportError::Closed;
        let lsp_err: LspError = transport_err.into();
        assert!(matches!(lsp_err, LspError::TransportError(_)));
        assert!(format!("{lsp_err}").contains("Connection closed"));
    }

    #[test]
    fn test_from_serde_json_error() {
        let json_err = serde_json::from_str::<serde_json::Value>("invalid").unwrap_err();
        let lsp_err: LspError = json_err.into();
        assert!(matches!(lsp_err, LspError::Serialization(_)));
    }
}
