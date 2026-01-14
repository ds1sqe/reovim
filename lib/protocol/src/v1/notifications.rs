//! Notification type names and payloads.
//!
//! This module defines notification types sent by the server to clients.
//! Notifications are one-way messages that don't expect a response.

use serde::{Deserialize, Serialize};

use super::types::Position;

// Notification type name constants

/// Mode changed notification.
pub const MODE_CHANGED: &str = "notification/mode_changed";

/// Cursor moved notification.
pub const CURSOR_MOVED: &str = "notification/cursor_moved";

/// Buffer modified notification.
pub const BUFFER_MODIFIED: &str = "notification/buffer_modified";

/// Render complete notification.
pub const RENDER_COMPLETE: &str = "notification/render_complete";

// Notification payload types

/// Payload for mode changed notification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModeChangedPayload {
    /// The new mode information.
    pub mode: super::types::ModeInfo,
}

impl ModeChangedPayload {
    /// Convert payload to JSON-RPC notification.
    ///
    /// # Panics
    ///
    /// This function will not panic as `ModeChangedPayload` serialization is infallible.
    #[must_use]
    pub fn into_notification(self) -> super::messages::RpcNotification {
        super::messages::RpcNotification::new(
            MODE_CHANGED,
            serde_json::to_value(self).expect("ModeChangedPayload serialization cannot fail"),
        )
    }
}

/// Payload for cursor moved notification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CursorMovedPayload {
    /// The buffer where cursor moved.
    pub buffer_id: usize,
    /// The new cursor position.
    pub position: Position,
}

impl CursorMovedPayload {
    /// Convert payload to JSON-RPC notification.
    ///
    /// # Panics
    ///
    /// This function will not panic as `CursorMovedPayload` serialization is infallible.
    #[must_use]
    pub fn into_notification(self) -> super::messages::RpcNotification {
        super::messages::RpcNotification::new(
            CURSOR_MOVED,
            serde_json::to_value(self).expect("CursorMovedPayload serialization cannot fail"),
        )
    }
}

/// Payload for buffer modified notification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BufferModifiedPayload {
    /// The buffer that was modified.
    pub buffer_id: usize,
    /// Whether the buffer has unsaved changes.
    pub modified: bool,
}

impl BufferModifiedPayload {
    /// Convert payload to JSON-RPC notification.
    ///
    /// # Panics
    ///
    /// This function will not panic as `BufferModifiedPayload` serialization is infallible.
    #[must_use]
    pub fn into_notification(self) -> super::messages::RpcNotification {
        super::messages::RpcNotification::new(
            BUFFER_MODIFIED,
            serde_json::to_value(self).expect("BufferModifiedPayload serialization cannot fail"),
        )
    }
}

/// Payload for render complete notification.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RenderCompletePayload {
    // Empty for now, can add timing info later
}

impl RenderCompletePayload {
    /// Convert payload to JSON-RPC notification.
    ///
    /// # Panics
    ///
    /// This function will not panic as `RenderCompletePayload` serialization is infallible.
    #[must_use]
    pub fn into_notification(self) -> super::messages::RpcNotification {
        super::messages::RpcNotification::new(
            RENDER_COMPLETE,
            serde_json::to_value(self).expect("RenderCompletePayload serialization cannot fail"),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_notification_names_format() {
        assert!(MODE_CHANGED.starts_with("notification/"));
        assert!(CURSOR_MOVED.starts_with("notification/"));
        assert!(BUFFER_MODIFIED.starts_with("notification/"));
        assert!(RENDER_COMPLETE.starts_with("notification/"));
    }

    #[test]
    fn test_cursor_moved_serialization() {
        let payload = CursorMovedPayload {
            buffer_id: 1,
            position: Position {
                line: 10,
                column: 5,
            },
        };
        let json = serde_json::to_string(&payload).unwrap();
        assert!(json.contains("\"buffer_id\":1"));
        assert!(json.contains("\"line\":10"));
        assert!(json.contains("\"column\":5"));
    }

    #[test]
    fn test_mode_changed_into_notification() {
        let payload = ModeChangedPayload {
            mode: super::super::types::ModeInfo::default(),
        };
        let notification = payload.into_notification();
        assert_eq!(notification.jsonrpc, "2.0");
        assert_eq!(notification.method, MODE_CHANGED);
    }

    #[test]
    fn test_cursor_moved_into_notification() {
        let payload = CursorMovedPayload {
            buffer_id: 1,
            position: Position::new(10, 5),
        };
        let notification = payload.into_notification();
        assert_eq!(notification.jsonrpc, "2.0");
        assert_eq!(notification.method, CURSOR_MOVED);
        assert!(notification.params.get("buffer_id").is_some());
    }

    #[test]
    fn test_buffer_modified_into_notification() {
        let payload = BufferModifiedPayload {
            buffer_id: 1,
            modified: true,
        };
        let notification = payload.into_notification();
        assert_eq!(notification.jsonrpc, "2.0");
        assert_eq!(notification.method, BUFFER_MODIFIED);
    }

    #[test]
    fn test_render_complete_into_notification() {
        let payload = RenderCompletePayload::default();
        let notification = payload.into_notification();
        assert_eq!(notification.jsonrpc, "2.0");
        assert_eq!(notification.method, RENDER_COMPLETE);
    }
}
