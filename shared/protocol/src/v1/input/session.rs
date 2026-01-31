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
mod tests {
    use super::*;

    #[test]
    fn test_attach_event_minimal() {
        let attach = AttachEvent::new();
        let json = serde_json::to_string(&attach).unwrap();
        // All None fields should be skipped
        assert_eq!(json, "{}");
    }

    #[test]
    fn test_attach_event_with_session() {
        let attach = AttachEvent::with_session("session-123");
        let json = serde_json::to_string(&attach).unwrap();
        assert!(json.contains("\"session_id\":\"session-123\""));
        assert!(!json.contains("\"client_id\""));
    }

    #[test]
    fn test_attach_event_with_size() {
        let attach = AttachEvent::with_size(80, 24);
        let json = serde_json::to_string(&attach).unwrap();
        assert!(json.contains("\"width\":80"));
        assert!(json.contains("\"height\":24"));
    }

    #[test]
    fn test_detach_event_normal() {
        let detach = DetachEvent::normal();
        let json = serde_json::to_string(&detach).unwrap();
        assert!(json.contains("\"reason\":\"normal\""));
        assert!(!json.contains("\"message\""));
    }

    #[test]
    fn test_detach_event_with_reason() {
        let detach = DetachEvent::with_reason(DetachReason::Kicked);
        let json = serde_json::to_string(&detach).unwrap();
        assert!(json.contains("\"reason\":\"kicked\""));
    }

    #[test]
    fn test_detach_event_with_message() {
        let detach = DetachEvent::with_message(DetachReason::SessionClosed, "Server shutting down");
        let json = serde_json::to_string(&detach).unwrap();
        assert!(json.contains("\"reason\":\"session_closed\""));
        assert!(json.contains("\"message\":\"Server shutting down\""));
    }

    #[test]
    fn test_detach_reason_serialization() {
        assert_eq!(serde_json::to_string(&DetachReason::Normal).unwrap(), "\"normal\"");
        assert_eq!(serde_json::to_string(&DetachReason::Disconnected).unwrap(), "\"disconnected\"");
        assert_eq!(
            serde_json::to_string(&DetachReason::SessionClosed).unwrap(),
            "\"session_closed\""
        );
        assert_eq!(serde_json::to_string(&DetachReason::Kicked).unwrap(), "\"kicked\"");
    }
}
