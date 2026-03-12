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

// Additional coverage tests

#[test]
fn test_log_entry_constant() {
    assert!(LOG_ENTRY.starts_with("notification/"));
    assert_eq!(LOG_ENTRY, "notification/log_entry");
}

#[test]
fn test_log_source_default() {
    let source = LogSource::default();
    assert_eq!(source, LogSource::Server);
}

#[test]
fn test_log_level_default() {
    let level = LogLevel::default();
    assert_eq!(level, LogLevel::Info);
}

#[test]
fn test_log_level_serialization() {
    assert_eq!(serde_json::to_string(&LogLevel::Trace).unwrap(), "\"trace\"");
    assert_eq!(serde_json::to_string(&LogLevel::Debug).unwrap(), "\"debug\"");
    assert_eq!(serde_json::to_string(&LogLevel::Info).unwrap(), "\"info\"");
    assert_eq!(serde_json::to_string(&LogLevel::Warn).unwrap(), "\"warn\"");
    assert_eq!(serde_json::to_string(&LogLevel::Error).unwrap(), "\"error\"");
}

#[test]
fn test_log_level_deserialization() {
    let level: LogLevel = serde_json::from_str("\"trace\"").unwrap();
    assert_eq!(level, LogLevel::Trace);
    let level: LogLevel = serde_json::from_str("\"error\"").unwrap();
    assert_eq!(level, LogLevel::Error);
}

#[test]
fn test_log_level_from_str_lossy_empty() {
    // Empty string defaults to Info
    assert_eq!(LogLevel::from_str_lossy(""), LogLevel::Info);
}

#[test]
fn test_log_level_from_str_lossy_info_explicit() {
    assert_eq!(LogLevel::from_str_lossy("info"), LogLevel::Info);
    assert_eq!(LogLevel::from_str_lossy("INFO"), LogLevel::Info);
}

#[test]
fn test_log_source_deserialization() {
    let source: LogSource = serde_json::from_str("\"server\"").unwrap();
    assert_eq!(source, LogSource::Server);
    let source: LogSource = serde_json::from_str("\"client\"").unwrap();
    assert_eq!(source, LogSource::Client);
}

#[test]
fn test_log_entry_payload_roundtrip() {
    let payload = LogEntryPayload {
        timestamp: "2026-01-01T00:00:00Z".to_string(),
        level: LogLevel::Error,
        target: "reovim::server".to_string(),
        message: "something failed".to_string(),
        source: LogSource::Client,
    };
    let json = serde_json::to_string(&payload).unwrap();
    let decoded: LogEntryPayload = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded.level, LogLevel::Error);
    assert_eq!(decoded.source, LogSource::Client);
    assert_eq!(decoded.target, "reovim::server");
}

#[test]
fn test_render_complete_payload_default() {
    let payload = RenderCompletePayload::default();
    let json = serde_json::to_string(&payload).unwrap();
    assert_eq!(json, "{}");
}

#[test]
fn test_detach_payload_default() {
    let payload = DetachPayload::default();
    assert!(payload.reason.is_none());
}

#[test]
fn test_wire_cmdline_prompt_char() {
    assert_eq!(WireCmdlinePrompt::Command.char(), ':');
    assert_eq!(WireCmdlinePrompt::SearchForward.char(), '/');
    assert_eq!(WireCmdlinePrompt::SearchBackward.char(), '?');
}

#[test]
fn test_wire_cmdline_prompt_default() {
    let prompt = WireCmdlinePrompt::default();
    assert_eq!(prompt, WireCmdlinePrompt::Command);
}

#[test]
fn test_wire_cmdline_prompt_serialization() {
    assert_eq!(serde_json::to_string(&WireCmdlinePrompt::Command).unwrap(), "\"command\"");
    assert_eq!(
        serde_json::to_string(&WireCmdlinePrompt::SearchForward).unwrap(),
        "\"search_forward\""
    );
    assert_eq!(
        serde_json::to_string(&WireCmdlinePrompt::SearchBackward).unwrap(),
        "\"search_backward\""
    );
}

#[test]
fn test_cmdline_changed_payload_show() {
    let payload =
        CmdlineChangedPayload::show(WireCmdlinePrompt::SearchForward, "pattern".to_string(), 7);
    assert!(payload.visible);
    assert_eq!(payload.prompt, WireCmdlinePrompt::SearchForward);
    assert_eq!(payload.input, "pattern");
    assert_eq!(payload.cursor, 7);
}

#[test]
fn test_cmdline_changed_payload_hide() {
    let payload = CmdlineChangedPayload::hide();
    assert!(!payload.visible);
    assert_eq!(payload.prompt, WireCmdlinePrompt::Command);
    assert!(payload.input.is_empty());
    assert_eq!(payload.cursor, 0);
}

#[test]
fn test_cmdline_changed_payload_serialization() {
    let payload = CmdlineChangedPayload::show(WireCmdlinePrompt::Command, "w".to_string(), 1);
    let json = serde_json::to_string(&payload).unwrap();
    assert!(json.contains("\"visible\":true"));
    assert!(json.contains("\"prompt\":\"command\""));
    assert!(json.contains("\"input\":\"w\""));
    assert!(json.contains("\"cursor\":1"));
}

#[test]
fn test_cmdline_changed_payload_roundtrip() {
    let payload = CmdlineChangedPayload::show(
        WireCmdlinePrompt::SearchBackward,
        "search term".to_string(),
        11,
    );
    let json = serde_json::to_string(&payload).unwrap();
    let decoded: CmdlineChangedPayload = serde_json::from_str(&json).unwrap();
    assert!(decoded.visible);
    assert_eq!(decoded.prompt, WireCmdlinePrompt::SearchBackward);
    assert_eq!(decoded.input, "search term");
    assert_eq!(decoded.cursor, 11);
}

#[test]
fn test_cmdline_changed_payload_into_notification() {
    let payload =
        CmdlineChangedPayload::show(WireCmdlinePrompt::Command, "quit".to_string(), 4);
    let notification = payload.into_notification();
    assert_eq!(notification.jsonrpc, "2.0");
    assert_eq!(notification.method, CMDLINE_CHANGED);
}

#[test]
fn test_cmdline_changed_constant() {
    assert_eq!(CMDLINE_CHANGED, "notification/cmdline_changed");
}

#[test]
fn test_capture_request_constant_format() {
    assert!(CAPTURE_REQUEST.starts_with("tui/"));
    assert!(CAPTURE_RESPONSE.starts_with("tui/"));
}

#[test]
fn test_log_level_equality() {
    assert_eq!(LogLevel::Trace, LogLevel::Trace);
    assert_ne!(LogLevel::Trace, LogLevel::Error);
}

#[test]
fn test_mode_changed_payload_roundtrip() {
    let payload = ModeChangedPayload {
        mode: super::super::types::ModeInfo {
            focus: "Editor".to_string(),
            edit_mode: "Insert".to_string(),
            sub_mode: "None".to_string(),
            display: "INSERT".to_string(),
        },
    };
    let json = serde_json::to_string(&payload).unwrap();
    let decoded: ModeChangedPayload = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded.mode.edit_mode, "Insert");
}

#[test]
fn test_buffer_modified_payload_false() {
    let payload = BufferModifiedPayload {
        buffer_id: BufferId::from(0),
        modified: false,
    };
    let json = serde_json::to_string(&payload).unwrap();
    assert!(json.contains("\"modified\":false"));
}
