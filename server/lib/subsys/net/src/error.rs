//! Network subsystem error types.

use std::fmt;

use crate::transport::TransportKind;

/// Errors raised by subsys-net contracts and the drivers that
/// implement them.
#[derive(Debug)]
pub enum NetError {
    /// Failed to bind the listener.
    BindFailed(String),

    /// Failed to accept an incoming connection.
    AcceptFailed(String),

    /// The server stopped with a transport-level error.
    ServeFailed(String),

    /// Generic I/O error.
    Io(String),

    /// No free port found in the allocator's fallback range.
    PortExhausted,

    /// Address string is not parseable.
    InvalidAddress(String),

    /// Driver cannot serve the requested transport kind.
    UnsupportedTransport(TransportKind),
}

impl fmt::Display for NetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BindFailed(msg) => write!(f, "failed to bind: {msg}"),
            Self::AcceptFailed(msg) => write!(f, "failed to accept connection: {msg}"),
            Self::ServeFailed(msg) => write!(f, "serve failed: {msg}"),
            Self::Io(msg) => write!(f, "I/O error: {msg}"),
            Self::PortExhausted => write!(f, "no available ports in fallback range"),
            Self::InvalidAddress(msg) => write!(f, "invalid address: {msg}"),
            Self::UnsupportedTransport(kind) => {
                write!(f, "driver does not support transport: {kind:?}")
            }
        }
    }
}

impl std::error::Error for NetError {}

impl From<std::io::Error> for NetError {
    fn from(err: std::io::Error) -> Self {
        Self::Io(err.to_string())
    }
}

impl From<tonic::transport::Error> for NetError {
    fn from(err: tonic::transport::Error) -> Self {
        Self::ServeFailed(err.to_string())
    }
}

#[cfg(test)]
#[path = "error_tests.rs"]
mod tests;
