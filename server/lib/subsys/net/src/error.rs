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
#[path = "error_tests.rs"]
mod tests;
