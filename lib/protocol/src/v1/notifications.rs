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
mod tests {
    use super::*;

    #[test]
    fn test_notification_names_format() {
        assert!(MODE_CHANGED.starts_with("notification/"));
        assert!(CURSOR_MOVED.starts_with("notification/"));
        assert!(BUFFER_MODIFIED.starts_with("notification/"));
        assert!(RENDER_COMPLETE.starts_with("notification/"));
        assert!(DETACH.starts_with("notification/"));
    }

    #[test]
    fn test_cursor_moved_serialization() {
        let payload = CursorMovedPayload {
            buffer_id: BufferId::from(1),
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
            buffer_id: BufferId::from(1),
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
            buffer_id: BufferId::from(1),
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

    // Phase 1 tests for #332

    #[test]
    fn test_log_entry_payload_serialization() {
        let payload = LogEntryPayload {
            timestamp: "2026-01-17T12:00:00Z".to_string(),
            level: LogLevel::Info,
            target: "runner::server".to_string(),
            message: "Test message".to_string(),
            source: LogSource::Server,
        };
        let json = serde_json::to_string(&payload).unwrap();
        assert!(json.contains("\"timestamp\":\"2026-01-17T12:00:00Z\""));
        assert!(json.contains("\"level\":\"info\""));
        assert!(json.contains("\"target\":\"runner::server\""));
        assert!(json.contains("\"message\":\"Test message\""));
        assert!(json.contains("\"source\":\"server\""));
    }

    #[test]
    fn test_log_source_serialization() {
        assert_eq!(serde_json::to_string(&LogSource::Server).unwrap(), "\"server\"");
        assert_eq!(serde_json::to_string(&LogSource::Client).unwrap(), "\"client\"");
    }

    #[test]
    fn test_log_level_ordering() {
        assert!(LogLevel::Trace < LogLevel::Debug);
        assert!(LogLevel::Debug < LogLevel::Info);
        assert!(LogLevel::Info < LogLevel::Warn);
        assert!(LogLevel::Warn < LogLevel::Error);
    }

    #[test]
    fn test_log_entry_into_notification() {
        let payload = LogEntryPayload {
            timestamp: "2026-01-17T12:00:00Z".to_string(),
            level: LogLevel::Warn,
            target: "test".to_string(),
            message: "warning".to_string(),
            source: LogSource::Client,
        };
        let notification = payload.into_notification();
        assert_eq!(notification.jsonrpc, "2.0");
        assert_eq!(notification.method, LOG_ENTRY);
    }

    #[test]
    fn test_log_entry_payload_missing_field_deserialization() {
        // Missing 'source' field should use default (Server)
        let json = r#"{"timestamp":"2026-01-17T12:00:00Z","level":"info","target":"test","message":"msg"}"#;
        let payload: LogEntryPayload = serde_json::from_str(json).unwrap();
        assert_eq!(payload.source, LogSource::Server);
    }

    #[test]
    fn test_log_level_from_str_lossy() {
        assert_eq!(LogLevel::from_str_lossy("trace"), LogLevel::Trace);
        assert_eq!(LogLevel::from_str_lossy("DEBUG"), LogLevel::Debug);
        assert_eq!(LogLevel::from_str_lossy("Info"), LogLevel::Info);
        assert_eq!(LogLevel::from_str_lossy("WARN"), LogLevel::Warn);
        assert_eq!(LogLevel::from_str_lossy("warning"), LogLevel::Warn);
        assert_eq!(LogLevel::from_str_lossy("error"), LogLevel::Error);
        assert_eq!(LogLevel::from_str_lossy("invalid"), LogLevel::Info); // fallback
    }

    #[test]
    fn test_log_entry_with_empty_strings() {
        let payload = LogEntryPayload {
            timestamp: String::new(),
            level: LogLevel::Debug,
            target: String::new(),
            message: String::new(),
            source: LogSource::Server,
        };
        let json = serde_json::to_string(&payload).unwrap();
        assert!(json.contains("\"timestamp\":\"\""));
        assert!(json.contains("\"target\":\"\""));
        assert!(json.contains("\"message\":\"\""));
    }

    #[test]
    fn test_log_level_invalid_deserialization() {
        // Invalid level should fail deserialization
        let json = r#""invalid_level""#;
        let result: Result<LogLevel, _> = serde_json::from_str(json);
        assert!(result.is_err());
    }

    // Phase 5 tests for #350

    #[test]
    fn test_detach_payload_new() {
        let payload = DetachPayload::new();
        assert!(payload.reason.is_none());
    }

    #[test]
    fn test_detach_payload_with_reason() {
        let payload = DetachPayload::with_reason("user requested");
        assert_eq!(payload.reason, Some("user requested".to_string()));
    }

    #[test]
    fn test_detach_payload_serialization() {
        let payload = DetachPayload::new();
        let json = serde_json::to_string(&payload).unwrap();
        // Empty object when no reason
        assert_eq!(json, "{}");

        let payload = DetachPayload::with_reason("test");
        let json = serde_json::to_string(&payload).unwrap();
        assert!(json.contains("\"reason\":\"test\""));
    }

    #[test]
    fn test_detach_payload_deserialization() {
        let json = "{}";
        let payload: DetachPayload = serde_json::from_str(json).unwrap();
        assert!(payload.reason.is_none());

        let json = r#"{"reason":"detached"}"#;
        let payload: DetachPayload = serde_json::from_str(json).unwrap();
        assert_eq!(payload.reason, Some("detached".to_string()));
    }

    #[test]
    fn test_detach_payload_into_notification() {
        let payload = DetachPayload::new();
        let notification = payload.into_notification();
        assert_eq!(notification.jsonrpc, "2.0");
        assert_eq!(notification.method, DETACH);
    }

    // Layout changed notification tests (#444)

    #[test]
    fn test_layout_changed_constant() {
        assert!(LAYOUT_CHANGED.starts_with("notification/"));
        assert_eq!(LAYOUT_CHANGED, "notification/layout_changed");
    }

    #[test]
    fn test_layout_changed_payload_split() {
        use super::super::types::{
            WireLayoutChangeKind, WireLayoutInfo, WireSplitDirection, WireWindowId,
        };

        let kind = WireLayoutChangeKind::Split {
            new_window: WireWindowId(1),
            direction: WireSplitDirection::Vertical,
        };
        let layout = WireLayoutInfo::single_window(80, 24, None);
        let payload = LayoutChangedPayload::new(kind, layout);

        let json = serde_json::to_string(&payload).unwrap();
        assert!(json.contains("\"kind\""));
        assert!(json.contains("\"layout\""));
        assert!(json.contains("\"type\":\"split\""));
    }

    #[test]
    fn test_layout_changed_payload_into_notification() {
        use super::super::types::{WireLayoutChangeKind, WireLayoutInfo};

        let kind = WireLayoutChangeKind::Equalize;
        let layout = WireLayoutInfo::default();
        let payload = LayoutChangedPayload::new(kind, layout);

        let notification = payload.into_notification();
        assert_eq!(notification.jsonrpc, "2.0");
        assert_eq!(notification.method, LAYOUT_CHANGED);
    }

    #[test]
    fn test_layout_changed_payload_roundtrip() {
        use super::super::types::{WireLayoutChangeKind, WireLayoutInfo, WireWindowId};

        let kind = WireLayoutChangeKind::Focus {
            from: Some(WireWindowId(0)),
            to: WireWindowId(1),
        };
        let layout = WireLayoutInfo::single_window(120, 40, None);
        let payload = LayoutChangedPayload::new(kind, layout);

        let json = serde_json::to_string(&payload).unwrap();
        let parsed: LayoutChangedPayload = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.layout.window_count, 1);
    }

    // TUI capture notification tests (#447)

    #[test]
    fn test_capture_request_constant() {
        assert_eq!(CAPTURE_REQUEST, "tui/capture-request");
    }

    #[test]
    fn test_capture_response_constant() {
        assert_eq!(CAPTURE_RESPONSE, "tui/capture-response");
    }

    #[test]
    fn test_capture_request_payload_new() {
        let payload = CaptureRequestPayload::new(42, ScreenFormat::RawAnsi);
        assert_eq!(payload.request_id, 42);
        assert_eq!(payload.format, ScreenFormat::RawAnsi);
    }

    #[test]
    fn test_capture_request_payload_serialization() {
        let payload = CaptureRequestPayload::new(123, ScreenFormat::PlainText);
        let json = serde_json::to_string(&payload).unwrap();
        assert!(json.contains("\"request_id\":123"));
        assert!(json.contains("\"format\":\"plain_text\""));
    }

    #[test]
    fn test_capture_request_payload_roundtrip() {
        let payload = CaptureRequestPayload::new(42, ScreenFormat::RawAnsi);
        let json = serde_json::to_string(&payload).unwrap();
        let decoded: CaptureRequestPayload = serde_json::from_str(&json).unwrap();
        assert_eq!(payload, decoded);
    }

    #[test]
    fn test_capture_request_payload_default_format() {
        // Missing format should default to PlainText
        let json = r#"{"request_id":1}"#;
        let payload: CaptureRequestPayload = serde_json::from_str(json).unwrap();
        assert_eq!(payload.format, ScreenFormat::PlainText);
    }

    #[test]
    fn test_capture_request_payload_into_notification() {
        let payload = CaptureRequestPayload::new(1, ScreenFormat::RawAnsi);
        let notification = payload.into_notification();
        assert_eq!(notification.jsonrpc, "2.0");
        assert_eq!(notification.method, CAPTURE_REQUEST);
    }

    #[test]
    fn test_capture_response_payload_new() {
        let result = ScreenContentResult {
            width: 80,
            height: 24,
            format: ScreenFormat::RawAnsi,
            content: "test content".to_string(),
        };
        let payload = CaptureResponsePayload::new(42, result);
        assert_eq!(payload.request_id, 42);
        assert_eq!(payload.result.width, 80);
        assert_eq!(payload.result.content, "test content");
    }

    #[test]
    fn test_capture_response_payload_serialization() {
        let result = ScreenContentResult {
            width: 120,
            height: 40,
            format: ScreenFormat::PlainText,
            content: "hello world".to_string(),
        };
        let payload = CaptureResponsePayload::new(123, result);
        let json = serde_json::to_string(&payload).unwrap();
        assert!(json.contains("\"request_id\":123"));
        assert!(json.contains("\"width\":120"));
        assert!(json.contains("\"height\":40"));
        assert!(json.contains("\"content\":\"hello world\""));
    }

    #[test]
    fn test_capture_response_payload_roundtrip() {
        let result = ScreenContentResult {
            width: 80,
            height: 24,
            format: ScreenFormat::CellGrid,
            content: "{}".to_string(),
        };
        let payload = CaptureResponsePayload::new(42, result);
        let json = serde_json::to_string(&payload).unwrap();
        let decoded: CaptureResponsePayload = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.request_id, 42);
        assert_eq!(decoded.result.width, 80);
    }

    #[test]
    fn test_capture_response_payload_into_notification() {
        let result = ScreenContentResult {
            width: 80,
            height: 24,
            format: ScreenFormat::PlainText,
            content: String::new(),
        };
        let payload = CaptureResponsePayload::new(1, result);
        let notification = payload.into_notification();
        assert_eq!(notification.jsonrpc, "2.0");
        assert_eq!(notification.method, CAPTURE_RESPONSE);
    }

    // Option changed notification tests (#445)

    #[test]
    fn test_option_changed_constant() {
        assert!(OPTION_CHANGED.starts_with("notification/"));
        assert_eq!(OPTION_CHANGED, "notification/option_changed");
    }

    #[test]
    fn test_option_changed_payload_global() {
        let payload = OptionChangedPayload::global("number", serde_json::json!(true));
        assert_eq!(payload.name, "number");
        assert_eq!(payload.value, serde_json::json!(true));
        assert!(payload.window_id.is_none());
    }

    #[test]
    fn test_option_changed_payload_window() {
        let payload = OptionChangedPayload::window("relativenumber", serde_json::json!(true), 42);
        assert_eq!(payload.name, "relativenumber");
        assert_eq!(payload.value, serde_json::json!(true));
        assert_eq!(payload.window_id, Some(42));
    }

    #[test]
    fn test_option_changed_payload_serialization() {
        // Global option (no window_id in JSON)
        let payload = OptionChangedPayload::global("number", serde_json::json!(true));
        let json = serde_json::to_string(&payload).unwrap();
        assert!(json.contains("\"name\":\"number\""));
        assert!(json.contains("\"value\":true"));
        assert!(!json.contains("window_id")); // Skipped when None

        // Window-scoped option
        let payload = OptionChangedPayload::window("tabstop", serde_json::json!(4), 7);
        let json = serde_json::to_string(&payload).unwrap();
        assert!(json.contains("\"name\":\"tabstop\""));
        assert!(json.contains("\"value\":4"));
        assert!(json.contains("\"window_id\":7"));
    }

    #[test]
    fn test_option_changed_payload_roundtrip() {
        let payload = OptionChangedPayload::window("shiftwidth", serde_json::json!(2), 3);
        let json = serde_json::to_string(&payload).unwrap();
        let decoded: OptionChangedPayload = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.name, "shiftwidth");
        assert_eq!(decoded.value, serde_json::json!(2));
        assert_eq!(decoded.window_id, Some(3));
    }

    #[test]
    fn test_option_changed_payload_into_notification() {
        let payload = OptionChangedPayload::global("number", serde_json::json!(false));
        let notification = payload.into_notification();
        assert_eq!(notification.jsonrpc, "2.0");
        assert_eq!(notification.method, OPTION_CHANGED);
    }

    #[test]
    fn test_option_changed_payload_string_value() {
        let payload = OptionChangedPayload::global("colorscheme", serde_json::json!("gruvbox"));
        let json = serde_json::to_string(&payload).unwrap();
        assert!(json.contains("\"value\":\"gruvbox\""));
    }
}
