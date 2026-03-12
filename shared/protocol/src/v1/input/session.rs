//! Session event types for RPC protocol.
//!
//! Client attach/detach events for multi-client scenarios.

use serde::{Deserialize, Serialize};

/// Session attach event (serializable).
///
/// Sent when a client attaches to a session. This enables multi-client
/// scenarios where multiple terminals can connect to the same editor.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AttachEvent {
    /// Session ID to attach to (optional, server may assign).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    /// Client identifier for tracking.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_id: Option<String>,
    /// Requested terminal width.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub width: Option<u16>,
    /// Requested terminal height.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub height: Option<u16>,
}

impl AttachEvent {
    /// Create a new attach event.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            session_id: None,
            client_id: None,
            width: None,
            height: None,
        }
    }

    /// Create an attach event with session ID.
    #[must_use]
    pub fn with_session(session_id: impl Into<String>) -> Self {
        Self {
            session_id: Some(session_id.into()),
            client_id: None,
            width: None,
            height: None,
        }
    }

    /// Create an attach event with size.
    #[must_use]
    pub const fn with_size(width: u16, height: u16) -> Self {
        Self {
            session_id: None,
            client_id: None,
            width: Some(width),
            height: Some(height),
        }
    }
}

impl Default for AttachEvent {
    fn default() -> Self {
        Self::new()
    }
}

/// Detach reason.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DetachReason {
    /// Normal detach (user requested).
    #[default]
    Normal,
    /// Client disconnected unexpectedly.
    Disconnected,
    /// Session is being closed.
    SessionClosed,
    /// Client was kicked by another client.
    Kicked,
}

/// Session detach event (serializable).
///
/// Sent when a client detaches from a session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DetachEvent {
    /// Reason for detach.
    #[serde(default)]
    pub reason: DetachReason,
    /// Optional message explaining the detach.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

impl DetachEvent {
    /// Create a normal detach event.
    #[must_use]
    pub const fn normal() -> Self {
        Self {
            reason: DetachReason::Normal,
            message: None,
        }
    }

    /// Create a detach event with reason.
    #[must_use]
    pub const fn with_reason(reason: DetachReason) -> Self {
        Self {
            reason,
            message: None,
        }
    }

    /// Create a detach event with message.
    #[must_use]
    pub fn with_message(reason: DetachReason, message: impl Into<String>) -> Self {
        Self {
            reason,
            message: Some(message.into()),
        }
    }
}

impl Default for DetachEvent {
    fn default() -> Self {
        Self::normal()
    }
}

#[cfg(test)]
#[path = "session_tests.rs"]
mod tests;
