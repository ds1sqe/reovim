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

/// Payload for cursor moved notification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CursorMovedPayload {
    /// The buffer where cursor moved.
    pub buffer_id: usize,
    /// The new cursor position.
    pub position: Position,
}

/// Payload for buffer modified notification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BufferModifiedPayload {
    /// The buffer that was modified.
    pub buffer_id: usize,
    /// Whether the buffer has unsaved changes.
    pub modified: bool,
}

/// Payload for render complete notification.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RenderCompletePayload {
    // Empty for now, can add timing info later
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
}
