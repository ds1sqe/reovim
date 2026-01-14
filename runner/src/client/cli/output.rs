//! Output formatting for CLI client.
//!
//! Supports plain text (with colors) and JSON output.

use std::fmt::Write;

use serde_json::Value;

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
}
