//! Notification type names and payloads.
//!
//! This module defines notification types sent by the server to clients.
//! Notifications are one-way messages that don't expect a response.

use serde::{Deserialize, Serialize};

use super::{
    results::ScreenContentResult,
    types::{BufferId, Position, ScreenFormat, WireLayoutChangeKind, WireLayoutInfo},
};

// Notification type name constants

/// Mode changed notification.
pub const MODE_CHANGED: &str = "notification/mode_changed";

/// Cursor moved notification.
pub const CURSOR_MOVED: &str = "notification/cursor_moved";

/// Buffer modified notification.
pub const BUFFER_MODIFIED: &str = "notification/buffer_modified";

/// Render complete notification.
pub const RENDER_COMPLETE: &str = "notification/render_complete";

/// Log entry notification (for log streaming).
pub const LOG_ENTRY: &str = "notification/log_entry";

/// Detach notification (client should disconnect).
pub const DETACH: &str = "notification/detach";

/// Layout changed notification.
pub const LAYOUT_CHANGED: &str = "notification/layout_changed";

/// Option changed notification.
///
/// Sent to clients when an editor option changes.
pub const OPTION_CHANGED: &str = "notification/option_changed";

/// Cmdline changed notification (#451).
///
/// Sent to clients when cmdline state changes (show/hide/input update).
pub const CMDLINE_CHANGED: &str = "notification/cmdline_changed";

/// TUI capture request notification.
///
/// Sent by server to TUI client to request a frame capture.
pub const CAPTURE_REQUEST: &str = "tui/capture-request";

/// TUI capture response notification.
///
/// Sent by TUI client to server with the captured frame content.
pub const CAPTURE_RESPONSE: &str = "tui/capture-response";

// Notification payload types

/// Source of a log entry.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum LogSource {
    /// Log from the server.
    #[default]
    Server,
    /// Log from the client.
    Client,
}

/// Log level for filtering and display.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    /// Trace level (most verbose).
    Trace = 0,
    /// Debug level.
    Debug = 1,
    /// Info level.
    #[default]
    Info = 2,
    /// Warning level.
    Warn = 3,
    /// Error level (least verbose).
    Error = 4,
}

impl LogLevel {
    /// Parse a log level from a string.
    #[must_use]
    pub fn from_str_lossy(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "trace" => Self::Trace,
            "debug" => Self::Debug,
            "warn" | "warning" => Self::Warn,
            "error" => Self::Error,
            // "info" and anything else defaults to Info
            _ => Self::Info,
        }
    }
}

/// Payload for log entry notification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntryPayload {
    /// Timestamp in ISO 8601 format.
    pub timestamp: String,
    /// Log level.
    pub level: LogLevel,
    /// Target module.
    pub target: String,
    /// Log message.
    pub message: String,
    /// Source of the log entry.
    #[serde(default)]
    pub source: LogSource,
}

impl LogEntryPayload {
    /// Convert payload to JSON-RPC notification.
    ///
    /// # Panics
    ///
    /// This function will not panic as `LogEntryPayload` serialization is infallible.
    #[must_use]
    pub fn into_notification(self) -> super::messages::RpcNotification {
        super::messages::RpcNotification::new(
            LOG_ENTRY,
            serde_json::to_value(self).expect("LogEntryPayload serialization cannot fail"),
        )
    }
}

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
    pub buffer_id: BufferId,
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
    pub buffer_id: BufferId,
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

/// Payload for detach notification.
///
/// Sent to clients when they should disconnect gracefully.
/// The server continues running after detach.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DetachPayload {
    /// Optional reason for detach.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

impl DetachPayload {
    /// Create a new detach payload.
    #[must_use]
    pub const fn new() -> Self {
        Self { reason: None }
    }

    /// Create a detach payload with a reason.
    #[must_use]
    pub fn with_reason(reason: impl Into<String>) -> Self {
        Self {
            reason: Some(reason.into()),
        }
    }

    /// Convert payload to JSON-RPC notification.
    ///
    /// # Panics
    ///
    /// This function will not panic as `DetachPayload` serialization is infallible.
    #[must_use]
    pub fn into_notification(self) -> super::messages::RpcNotification {
        super::messages::RpcNotification::new(
            DETACH,
            serde_json::to_value(self).expect("DetachPayload serialization cannot fail"),
        )
    }
}

/// Payload for layout changed notification.
///
/// Sent to clients when the window layout changes (split, close, focus, resize).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayoutChangedPayload {
    /// Type of layout change that occurred.
    pub kind: WireLayoutChangeKind,
    /// Full layout state after the change.
    pub layout: WireLayoutInfo,
}

impl LayoutChangedPayload {
    /// Create a new layout changed payload.
    #[must_use]
    pub const fn new(kind: WireLayoutChangeKind, layout: WireLayoutInfo) -> Self {
        Self { kind, layout }
    }

    /// Convert payload to JSON-RPC notification.
    ///
    /// # Panics
    ///
    /// This function will not panic as `LayoutChangedPayload` serialization is infallible.
    #[must_use]
    pub fn into_notification(self) -> super::messages::RpcNotification {
        super::messages::RpcNotification::new(
            LAYOUT_CHANGED,
            serde_json::to_value(self).expect("LayoutChangedPayload serialization cannot fail"),
        )
    }
}

/// Payload for option changed notification (#445).
///
/// Sent to clients when an editor option changes value.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptionChangedPayload {
    /// Name of the option that changed.
    pub name: String,
    /// New value of the option (bool, int, or string).
    pub value: serde_json::Value,
    /// Window ID if this is a window-scoped change, None for global.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub window_id: Option<usize>,
}

impl OptionChangedPayload {
    /// Create a new option changed payload for a global option.
    #[must_use]
    pub fn global(name: impl Into<String>, value: serde_json::Value) -> Self {
        Self {
            name: name.into(),
            value,
            window_id: None,
        }
    }

    /// Create a new option changed payload for a window-scoped option.
    #[must_use]
    pub fn window(name: impl Into<String>, value: serde_json::Value, window_id: usize) -> Self {
        Self {
            name: name.into(),
            value,
            window_id: Some(window_id),
        }
    }

    /// Convert payload to JSON-RPC notification.
    ///
    /// # Panics
    ///
    /// This function will not panic as `OptionChangedPayload` serialization is infallible.
    #[must_use]
    pub fn into_notification(self) -> super::messages::RpcNotification {
        super::messages::RpcNotification::new(
            OPTION_CHANGED,
            serde_json::to_value(self).expect("OptionChangedPayload serialization cannot fail"),
        )
    }
}

/// Prompt type for cmdline.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WireCmdlinePrompt {
    /// Ex command prompt (`:`)
    #[default]
    Command,
    /// Forward search prompt (`/`)
    SearchForward,
    /// Backward search prompt (`?`)
    SearchBackward,
}

impl WireCmdlinePrompt {
    /// Get the prompt character for display.
    #[must_use]
    pub const fn char(self) -> char {
        match self {
            Self::Command => ':',
            Self::SearchForward => '/',
            Self::SearchBackward => '?',
        }
    }
}

/// Payload for cmdline changed notification (#451).
///
/// Sent to clients when cmdline state changes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CmdlineChangedPayload {
    /// Whether cmdline is visible/active.
    pub visible: bool,
    /// The prompt type (`:`, `/`, `?`).
    pub prompt: WireCmdlinePrompt,
    /// Current input text.
    pub input: String,
    /// Cursor position within input.
    pub cursor: usize,
}

impl CmdlineChangedPayload {
    /// Create a payload for showing cmdline.
    #[must_use]
    #[allow(clippy::missing_const_for_fn)] // String is not const-compatible
    pub fn show(prompt: WireCmdlinePrompt, input: String, cursor: usize) -> Self {
        Self {
            visible: true,
            prompt,
            input,
            cursor,
        }
    }

    /// Create a payload for hiding cmdline.
    #[must_use]
    pub const fn hide() -> Self {
        Self {
            visible: false,
            prompt: WireCmdlinePrompt::Command,
            input: String::new(),
            cursor: 0,
        }
    }

    /// Convert payload to JSON-RPC notification.
    ///
    /// # Panics
    ///
    /// This function will not panic as `CmdlineChangedPayload` serialization is infallible.
    #[must_use]
    pub fn into_notification(self) -> super::messages::RpcNotification {
        super::messages::RpcNotification::new(
            CMDLINE_CHANGED,
            serde_json::to_value(self).expect("CmdlineChangedPayload serialization cannot fail"),
        )
    }
}

/// Payload for TUI capture request notification (#447).
///
/// Sent by server to TUI client to request a frame capture.
/// TUI responds with `CaptureResponsePayload`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CaptureRequestPayload {
    /// Correlation ID to match request with response.
    pub request_id: u64,
    /// Requested output format.
    #[serde(default)]
    pub format: ScreenFormat,
}

impl CaptureRequestPayload {
    /// Create a new capture request payload.
    #[must_use]
    pub const fn new(request_id: u64, format: ScreenFormat) -> Self {
        Self { request_id, format }
    }

    /// Convert payload to JSON-RPC notification.
    ///
    /// # Panics
    ///
    /// This function will not panic as `CaptureRequestPayload` serialization is infallible.
    #[must_use]
    pub fn into_notification(self) -> super::messages::RpcNotification {
        super::messages::RpcNotification::new(
            CAPTURE_REQUEST,
            serde_json::to_value(self).expect("CaptureRequestPayload serialization cannot fail"),
        )
    }
}

/// Payload for TUI capture response notification (#447).
///
/// Sent by TUI client to server with the captured frame content.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaptureResponsePayload {
    /// Correlation ID matching the original request.
    pub request_id: u64,
    /// Captured frame content.
    pub result: ScreenContentResult,
}

impl CaptureResponsePayload {
    /// Create a new capture response payload.
    #[must_use]
    pub const fn new(request_id: u64, result: ScreenContentResult) -> Self {
        Self { request_id, result }
    }

    /// Convert payload to JSON-RPC notification.
    ///
    /// # Panics
    ///
    /// This function will not panic as `CaptureResponsePayload` serialization is infallible.
    #[must_use]
    pub fn into_notification(self) -> super::messages::RpcNotification {
        super::messages::RpcNotification::new(
            CAPTURE_RESPONSE,
            serde_json::to_value(self).expect("CaptureResponsePayload serialization cannot fail"),
        )
    }
}

#[cfg(test)]
#[path = "notifications_tests.rs"]
mod tests;
