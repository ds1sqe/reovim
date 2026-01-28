//! Output formatting for CLI client.
//!
//! Supports plain text (with colors) and JSON output.

use std::fmt::Write;

use {
    reovim_protocol::v1::{
        BufferInfo, BufferListResult, CursorInfo, ModeInfo, ScreenContentResult, ScreenInfo,
    },
    serde_json::Value,
};

/// Output format.
#[derive(Debug, Clone, Copy, Default)]
pub enum OutputFormat {
    /// Human-readable plain text (with colors if tty).
    #[default]
    Plain,
    /// Machine-readable JSON.
    Json,
}

/// Format a JSON value for output.
#[must_use]
pub fn format_output(value: &Value, format: OutputFormat) -> String {
    match format {
        OutputFormat::Plain => format_plain(value),
        OutputFormat::Json => format_json(value),
    }
}

/// Format a JSON value for output with command context.
///
/// Uses typed deserialization based on the command to provide better formatting.
#[must_use]
pub fn format_output_for_command(value: &Value, format: OutputFormat, command: &str) -> String {
    match format {
        OutputFormat::Plain => format_plain_for_command(value, command),
        OutputFormat::Json => format_json(value),
    }
}

/// Format as plain text with command context for typed deserialization.
fn format_plain_for_command(value: &Value, command: &str) -> String {
    match command {
        "mode" => {
            if let Ok(mode) = serde_json::from_value::<ModeInfo>(value.clone()) {
                return format!("Mode: {}", mode.display);
            }
        }
        "cursor" => {
            if let Ok(cursor) = serde_json::from_value::<CursorInfo>(value.clone()) {
                return format!("Cursor: line {}, column {}", cursor.line, cursor.column);
            }
        }
        "screen" => {
            if let Ok(screen) = serde_json::from_value::<ScreenInfo>(value.clone()) {
                return format!("Screen: {}x{}", screen.width, screen.height);
            }
        }
        "content" => {
            // Handle screen content - return raw content (may contain ANSI)
            if let Ok(content) = serde_json::from_value::<ScreenContentResult>(value.clone()) {
                return content.content;
            }
        }
        "buffers" => {
            if let Ok(result) = serde_json::from_value::<BufferListResult>(value.clone()) {
                return format_buffer_list_typed(&result.buffers);
            }
        }
        "log-tail" => {
            return format_log_entries(value);
        }
        _ => {}
    }
    // Fallback to generic formatting
    format_plain(value)
}

/// Format log entries for plain text output with color-coded levels.
fn format_log_entries(value: &Value) -> String {
    let Some(entries) = value.get("entries").and_then(Value::as_array) else {
        return format_plain(value);
    };

    if entries.is_empty() {
        return "No log entries".to_string();
    }

    let overflow = value
        .get("overflow_count")
        .and_then(Value::as_u64)
        .unwrap_or(0);

    // Check if stdout is a tty for color support
    let use_color = std::io::IsTerminal::is_terminal(&std::io::stdout());

    let mut output = String::new();

    for entry in entries {
        let timestamp = entry.get("timestamp").and_then(Value::as_str).unwrap_or("");
        let level = entry.get("level").and_then(Value::as_str).unwrap_or("INFO");
        let target = entry.get("target").and_then(Value::as_str).unwrap_or("");
        let message = entry.get("message").and_then(Value::as_str).unwrap_or("");

        // Color-coded level
        let level_str = if use_color {
            match level.to_uppercase().as_str() {
                "ERROR" => format!("\x1b[31m{level:5}\x1b[0m"), // red
                "WARN" => format!("\x1b[33m{level:5}\x1b[0m"),  // yellow
                "INFO" => format!("\x1b[32m{level:5}\x1b[0m"),  // green
                "DEBUG" => format!("\x1b[36m{level:5}\x1b[0m"), // cyan
                "TRACE" => format!("\x1b[90m{level:5}\x1b[0m"), // gray
                _ => format!("{level:5}"),
            }
        } else {
            format!("{level:5}")
        };

        let _ = writeln!(output, "{timestamp} {level_str} {target}: {message}");
    }

    if overflow > 0 {
        let _ = writeln!(output, "\n({overflow} older entries dropped due to buffer overflow)");
    }

    output
}

/// Format buffer list from typed data.
fn format_buffer_list_typed(buffers: &[BufferInfo]) -> String {
    if buffers.is_empty() {
        return "No buffers".to_string();
    }

    let mut output = String::from("ID    Modified  Path\n");
    output.push_str("----  --------  ----\n");

    for buf in buffers {
        let mod_str = if buf.modified { "*" } else { " " };
        let path = buf.file_path.as_deref().unwrap_or("[No Name]");
        let _ = writeln!(output, "{:<4}  {:<8}  {}", buf.id, mod_str, path);
    }

    output
}

/// Format as pretty JSON.
fn format_json(value: &Value) -> String {
    serde_json::to_string_pretty(value).unwrap_or_else(|_| value.to_string())
}

/// Format as plain text.
fn format_plain(value: &Value) -> String {
    match value {
        Value::Null => "null".to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => n.to_string(),
        Value::String(s) => s.clone(),
        Value::Array(arr) => arr.iter().map(format_plain).collect::<Vec<_>>().join("\n"),
        Value::Object(obj) => {
            // Special handling for known response types
            if let Some(mode) = obj.get("display") {
                return format!("Mode: {}", mode.as_str().unwrap_or("unknown"));
            }
            if let (Some(line), Some(col)) = (obj.get("line"), obj.get("column")) {
                return format!(
                    "Cursor: ({}, {})",
                    line.as_u64().unwrap_or(0),
                    col.as_u64().unwrap_or(0)
                );
            }
            if let Some(content) = obj.get("content") {
                return content.as_str().unwrap_or("").to_string();
            }
            if let Some(buffers) = obj.get("buffers") {
                return format_buffer_list(buffers);
            }
            if let Some(modules) = obj.get("modules") {
                return format_module_list(modules);
            }

            // Generic object formatting
            obj.iter()
                .map(|(k, v)| format!("{k}: {}", format_plain(v)))
                .collect::<Vec<_>>()
                .join("\n")
        }
    }
}

/// Format buffer list.
fn format_buffer_list(buffers: &Value) -> String {
    if let Value::Array(arr) = buffers {
        if arr.is_empty() {
            return "No buffers".to_string();
        }

        let mut output = String::from("ID    Modified  Path\n");
        output.push_str("----  --------  ----\n");

        for buf in arr {
            let id = buf.get("id").and_then(Value::as_u64).unwrap_or(0);
            let modified = buf
                .get("modified")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            let path = buf
                .get("file_path")
                .and_then(Value::as_str)
                .unwrap_or("[No Name]");
            let mod_str = if modified { "*" } else { " " };
            let _ = writeln!(output, "{id:<4}  {mod_str:<8}  {path}");
        }

        output
    } else {
        "Invalid buffer list".to_string()
    }
}

/// Format module list.
fn format_module_list(modules: &Value) -> String {
    if let Value::Array(arr) = modules {
        if arr.is_empty() {
            return "No modules loaded".to_string();
        }

        let mut output = String::from("ID                    State     Static\n");
        output.push_str("--------------------  --------  ------\n");

        for module in arr {
            let id = module
                .get("id")
                .and_then(Value::as_str)
                .unwrap_or("unknown");
            let state = module
                .get("state")
                .and_then(Value::as_str)
                .unwrap_or("unknown");
            let is_static = module
                .get("is_static")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            let static_str = if is_static { "yes" } else { "no" };
            let _ = writeln!(output, "{id:<20}  {state:<8}  {static_str}");
        }

        output
    } else {
        "Invalid module list".to_string()
    }
}

/// Format mode response.
#[must_use]
pub fn format_mode(value: &Value) -> String {
    value
        .get("display")
        .and_then(Value::as_str)
        .map(|display| format!("Mode: {display}"))
        .or_else(|| {
            value
                .get("edit_mode")
                .and_then(Value::as_str)
                .map(|edit_mode| format!("Mode: {edit_mode}"))
        })
        .unwrap_or_else(|| format_plain(value))
}

/// Format cursor response.
#[must_use]
pub fn format_cursor(value: &Value) -> String {
    let line = value.get("line").and_then(Value::as_u64).unwrap_or(0);
    let col = value.get("column").and_then(Value::as_u64).unwrap_or(0);
    format!("Cursor: line {line}, column {col}")
}

/// Format screen content response.
#[must_use]
pub fn format_screen_content(value: &Value) -> String {
    value
        .get("content")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string()
}

#[cfg(test)]
mod tests {
    use {super::*, serde_json::json};

    #[test]
    fn test_format_mode() {
        let value = json!({"display": "NORMAL", "edit_mode": "Normal"});
        assert_eq!(format_mode(&value), "Mode: NORMAL");
    }

    #[test]
    fn test_format_cursor() {
        let value = json!({"line": 10, "column": 5});
        assert_eq!(format_cursor(&value), "Cursor: line 10, column 5");
    }

    #[test]
    fn test_format_json() {
        let value = json!({"key": "value"});
        let output = format_output(&value, OutputFormat::Json);
        assert!(output.contains("\"key\""));
        assert!(output.contains("\"value\""));
    }

    #[test]
    fn test_format_mode_typed() {
        let value = json!({
            "focus": "Editor",
            "edit_mode": "Normal",
            "sub_mode": "None",
            "display": "NORMAL"
        });
        let output = format_output_for_command(&value, OutputFormat::Plain, "mode");
        assert_eq!(output, "Mode: NORMAL");
    }

    #[test]
    fn test_format_cursor_typed() {
        let value = json!({"line": 42, "column": 10});
        let output = format_output_for_command(&value, OutputFormat::Plain, "cursor");
        assert_eq!(output, "Cursor: line 42, column 10");
    }

    #[test]
    fn test_format_screen_typed() {
        let value = json!({
            "width": 120,
            "height": 40,
            "active_buffer_id": 1,
            "window_count": 1
        });
        let output = format_output_for_command(&value, OutputFormat::Plain, "screen");
        assert_eq!(output, "Screen: 120x40");
    }

    #[test]
    fn test_format_content_ansi() {
        // Test raw ANSI content passthrough
        let ansi_content = "\x1b[32mHello\x1b[0m World";
        let value = json!({"content": ansi_content});
        let output = format_output_for_command(&value, OutputFormat::Plain, "content");
        assert_eq!(output, ansi_content);
    }

    #[test]
    fn test_format_buffers_typed() {
        let value = json!({
            "buffers": [
                {"id": 1, "file_path": "/tmp/test.txt", "modified": false, "line_count": 10},
                {"id": 2, "file_path": null, "modified": true, "line_count": 5}
            ]
        });
        let output = format_output_for_command(&value, OutputFormat::Plain, "buffers");
        assert!(output.contains("/tmp/test.txt"));
        assert!(output.contains("[No Name]"));
        assert!(output.contains('*')); // modified indicator
    }

    #[test]
    fn test_format_unknown_command_fallback() {
        let value = json!({"key": "value"});
        let output = format_output_for_command(&value, OutputFormat::Plain, "unknown");
        // Should fall back to generic formatting
        assert!(output.contains("key"));
        assert!(output.contains("value"));
    }

    #[test]
    fn test_format_json_passthrough() {
        let value = json!({"content": "test"});
        let output = format_output_for_command(&value, OutputFormat::Json, "content");
        // JSON format should not do typed formatting
        assert!(output.contains("\"content\""));
        assert!(output.contains("\"test\""));
    }

    #[test]
    fn test_format_log_entries() {
        let value = json!({
            "entries": [
                {
                    "timestamp": "2025-01-17T12:00:00Z",
                    "level": "INFO",
                    "target": "test::module",
                    "message": "Test message"
                },
                {
                    "timestamp": "2025-01-17T12:00:01Z",
                    "level": "ERROR",
                    "target": "test::module",
                    "message": "Error occurred"
                }
            ],
            "overflow_count": 0
        });
        let output = format_output_for_command(&value, OutputFormat::Plain, "log-tail");
        assert!(output.contains("Test message"));
        assert!(output.contains("Error occurred"));
        assert!(output.contains("test::module"));
    }

    #[test]
    fn test_format_log_entries_empty() {
        let value = json!({
            "entries": [],
            "overflow_count": 0
        });
        let output = format_output_for_command(&value, OutputFormat::Plain, "log-tail");
        assert_eq!(output, "No log entries");
    }

    #[test]
    fn test_format_log_entries_with_overflow() {
        let value = json!({
            "entries": [{
                "timestamp": "2025-01-17T12:00:00Z",
                "level": "INFO",
                "target": "test",
                "message": "msg"
            }],
            "overflow_count": 100
        });
        let output = format_output_for_command(&value, OutputFormat::Plain, "log-tail");
        assert!(output.contains("100 older entries dropped"));
    }
}
