//! Network driver error types.

use std::fmt;

/// Errors that can occur during network operations.
#[derive(Debug)]
pub enum NetError {
    /// Failed to bind to address.
    BindFailed(String),

    /// Failed to accept connection.
    AcceptFailed(String),

    /// Connection closed unexpectedly.
    ConnectionClosed,

    /// I/O error.
    Io(String),

    /// No more ports available in fallback range.
    PortExhausted,

    /// Invalid address format.
    InvalidAddress(String),

    /// Transport not initialized.
    NotInitialized,

    /// Transport already listening.
    AlreadyListening,

    /// Failed to read from transport.
    ReadFailed(String),

    /// Failed to write to transport.
    WriteFailed(String),

    /// JSON serialization/deserialization error.
    JsonError(String),

    /// Invalid RPC message format.
    InvalidMessage(String),
}

impl fmt::Display for NetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BindFailed(msg) => write!(f, "failed to bind: {msg}"),
            Self::AcceptFailed(msg) => write!(f, "failed to accept connection: {msg}"),
            Self::ConnectionClosed => write!(f, "connection closed"),
            Self::Io(msg) => write!(f, "I/O error: {msg}"),
            Self::PortExhausted => write!(f, "no available ports in fallback range"),
            Self::InvalidAddress(msg) => write!(f, "invalid address: {msg}"),
            Self::NotInitialized => write!(f, "transport not initialized"),
            Self::AlreadyListening => write!(f, "transport already listening"),
            Self::ReadFailed(msg) => write!(f, "failed to read: {msg}"),
            Self::WriteFailed(msg) => write!(f, "failed to write: {msg}"),
            Self::JsonError(msg) => write!(f, "JSON error: {msg}"),
            Self::InvalidMessage(msg) => write!(f, "invalid message: {msg}"),
        }
    }
}

impl std::error::Error for NetError {}

impl From<std::io::Error> for NetError {
    fn from(err: std::io::Error) -> Self {
        Self::Io(err.to_string())
    }
}

impl From<serde_json::Error> for NetError {
    fn from(err: serde_json::Error) -> Self {
        Self::JsonError(err.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        assert!(format!("{}", NetError::BindFailed("addr".into())).contains("addr"));
        assert_eq!(format!("{}", NetError::ConnectionClosed), "connection closed");
        assert_eq!(format!("{}", NetError::PortExhausted), "no available ports in fallback range");
    }

    #[test]
    fn test_all_display_variants() {
        assert!(
            NetError::AcceptFailed("test".into())
                .to_string()
                .contains("accept")
        );
        assert!(NetError::Io("test".into()).to_string().contains("I/O"));
        assert!(
            NetError::InvalidAddress("test".into())
                .to_string()
                .contains("invalid address")
        );
        assert!(
            NetError::NotInitialized
                .to_string()
                .contains("not initialized")
        );
        assert!(
            NetError::AlreadyListening
                .to_string()
                .contains("already listening")
        );
        assert!(
            NetError::ReadFailed("test".into())
                .to_string()
                .contains("read")
        );
        assert!(
            NetError::WriteFailed("test".into())
                .to_string()
                .contains("write")
        );
        assert!(
            NetError::JsonError("test".into())
                .to_string()
                .contains("JSON")
        );
        assert!(
            NetError::InvalidMessage("test".into())
                .to_string()
                .contains("invalid message")
        );
    }

    #[test]
    fn test_debug() {
        let err = NetError::ConnectionClosed;
        assert!(format!("{err:?}").contains("ConnectionClosed"));
    }

    #[test]
    fn test_error_trait() {
        fn assert_error<E: std::error::Error>() {}
        assert_error::<NetError>();
    }

    #[test]
    fn test_from_io_error() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "test");
        let net_err = NetError::from(io_err);
        assert!(matches!(net_err, NetError::Io(_)));
    }

    #[test]
    fn test_from_json_error() {
        let json_str = "invalid json {";
        let json_err: Result<serde_json::Value, _> = serde_json::from_str(json_str);
        let net_err = NetError::from(json_err.unwrap_err());
        assert!(matches!(net_err, NetError::JsonError(_)));
    }

    #[test]
    fn test_error_display_bind_failed() {
        let err = NetError::BindFailed("port 8080".to_string());
        assert_eq!(err.to_string(), "failed to bind: port 8080");
    }

    #[test]
    fn test_error_display_accept_failed() {
        let err = NetError::AcceptFailed("timeout".to_string());
        assert_eq!(err.to_string(), "failed to accept connection: timeout");
    }

    #[test]
    fn test_error_display_connection_closed() {
        let err = NetError::ConnectionClosed;
        assert_eq!(err.to_string(), "connection closed");
    }

    #[test]
    fn test_error_display_io() {
        let err = NetError::Io("broken pipe".to_string());
        assert_eq!(err.to_string(), "I/O error: broken pipe");
    }

    #[test]
    fn test_error_display_port_exhausted() {
        let err = NetError::PortExhausted;
        assert_eq!(err.to_string(), "no available ports in fallback range");
    }

    #[test]
    fn test_error_display_invalid_address() {
        let err = NetError::InvalidAddress("bad host".to_string());
        assert_eq!(err.to_string(), "invalid address: bad host");
    }

    #[test]
    fn test_error_display_not_initialized() {
        let err = NetError::NotInitialized;
        assert_eq!(err.to_string(), "transport not initialized");
    }

    #[test]
    fn test_error_display_already_listening() {
        let err = NetError::AlreadyListening;
        assert_eq!(err.to_string(), "transport already listening");
    }

    #[test]
    fn test_error_display_read_failed() {
        let err = NetError::ReadFailed("EOF".to_string());
        assert_eq!(err.to_string(), "failed to read: EOF");
    }

    #[test]
    fn test_error_display_write_failed() {
        let err = NetError::WriteFailed("broken".to_string());
        assert_eq!(err.to_string(), "failed to write: broken");
    }

    #[test]
    fn test_error_display_json_error() {
        let err = NetError::JsonError("parse failed".to_string());
        assert_eq!(err.to_string(), "JSON error: parse failed");
    }

    #[test]
    fn test_error_display_invalid_message() {
        let err = NetError::InvalidMessage("missing method".to_string());
        assert_eq!(err.to_string(), "invalid message: missing method");
    }

    #[test]
    fn test_from_io_error_preserves_message() {
        let io_err = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "access denied");
        let net_err = NetError::from(io_err);
        match net_err {
            NetError::Io(msg) => assert!(msg.contains("access denied")),
            other => panic!("Expected Io variant, got {other:?}"),
        }
    }

    #[test]
    fn test_from_json_error_preserves_message() {
        let json_err: serde_json::Error =
            serde_json::from_str::<i32>("\"not a number\"").unwrap_err();
        let net_err = NetError::from(json_err);
        match net_err {
            NetError::JsonError(msg) => assert!(!msg.is_empty()),
            other => panic!("Expected JsonError variant, got {other:?}"),
        }
    }

    #[test]
    fn test_all_variants_debug() {
        let variants: Vec<NetError> = vec![
            NetError::BindFailed("test".into()),
            NetError::AcceptFailed("test".into()),
            NetError::ConnectionClosed,
            NetError::Io("test".into()),
            NetError::PortExhausted,
            NetError::InvalidAddress("test".into()),
            NetError::NotInitialized,
            NetError::AlreadyListening,
            NetError::ReadFailed("test".into()),
            NetError::WriteFailed("test".into()),
            NetError::JsonError("test".into()),
            NetError::InvalidMessage("test".into()),
        ];
        for variant in variants {
            let debug_str = format!("{variant:?}");
            assert!(!debug_str.is_empty());
        }
    }
}
