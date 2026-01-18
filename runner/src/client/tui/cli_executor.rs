//! CLI command execution for embedded REPL.
//!
//! Adapts CLI commands for use with `RpcWriter` (async, fire-and-forget).
//! Supports both fire-and-forget commands and query commands with response
//! correlation via `CommandType` classification.

use serde_json::{Value, json};

use crate::client::{cli::output::OutputFormat, common::RpcWriter};

use super::cli_panel::CliResult;

/// Result of command classification.
pub enum CommandType {
    /// Fire-and-forget command (no response needed).
    FireAndForget,
    /// Query command that needs response correlation.
    Query {
        /// RPC method to call.
        method: &'static str,
        /// RPC parameters.
        params: Value,
        /// Command name for result formatting.
        format_key: &'static str,
    },
    /// Local command handled entirely in CLI panel.
    Local(CliResult),
}

/// Format a query result for CLI panel display.
///
/// Uses the standalone CLI output module's formatting logic.
#[must_use]
pub fn format_query_result(command: &str, value: &Value) -> String {
    use crate::client::cli::output::format_output_for_command;
    format_output_for_command(value, OutputFormat::Plain, command)
}

/// Classify a command without executing.
///
/// Determines whether a command is:
/// - A local command (help, clear, quit) that needs no RPC
/// - A fire-and-forget command (keys, resize, open, kill) that doesn't need response
/// - A query command (mode, cursor, buffers, etc.) that needs response correlation
#[must_use]
#[allow(clippy::too_many_lines)]
pub fn classify_command(input: &str) -> CommandType {
    let args: Vec<&str> = input.split_whitespace().collect();
    let cmd = args.first().copied().unwrap_or("");

    match cmd {
        // Local commands
        "help" | "?" => CommandType::Local(CliResult::Ok(HELP_TEXT.to_string())),
        "quit" | "exit" | "q" => {
            CommandType::Local(CliResult::Ok("Use Esc or <C-b>; to close panel".to_string()))
        }
        "clear" => CommandType::Local(CliResult::Ok("__CLEAR__".to_string())),
        "" => CommandType::Local(CliResult::Ok(String::new())),

        // Fire-and-forget commands
        "keys" | "k" | "resize" | "open" | "e" | "kill" => CommandType::FireAndForget,

        // Query commands
        "mode" => CommandType::Query {
            method: "state/mode",
            params: json!({}),
            format_key: "mode",
        },
        "cursor" => CommandType::Query {
            method: "state/cursor",
            params: json!({}),
            format_key: "cursor",
        },
        "screen" => CommandType::Query {
            method: "state/screen",
            params: json!({}),
            format_key: "screen",
        },
        "buffers" | "ls" => CommandType::Query {
            method: "buffer/list",
            params: json!({}),
            format_key: "buffers",
        },
        "buffer" => {
            let buffer_id = args.get(1).and_then(|s| s.parse::<u64>().ok());
            let params = buffer_id.map_or_else(|| json!({}), |id| json!({ "buffer_id": id }));
            CommandType::Query {
                method: "buffer/get_content",
                params,
                format_key: "buffer",
            }
        }
        "modules" => CommandType::Query {
            method: "module/list",
            params: json!({}),
            format_key: "modules",
        },
        "version" => CommandType::Query {
            method: "debug/version",
            params: json!({}),
            format_key: "version",
        },
        "uptime" => CommandType::Query {
            method: "debug/uptime",
            params: json!({}),
            format_key: "uptime",
        },
        "registers" => CommandType::Query {
            method: "debug/registers",
            params: json!({}),
            format_key: "registers",
        },
        "marks" => CommandType::Query {
            method: "debug/marks",
            params: json!({}),
            format_key: "marks",
        },
        "log-level" => {
            let level = args.get(1).copied();
            let params = level.map_or_else(|| json!({}), |l| json!({ "level": l }));
            CommandType::Query {
                method: "debug/log_level",
                params,
                format_key: "log-level",
            }
        }
        "log-tail" => {
            // Simple version: just count, no filters in embedded CLI
            let count = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(20);
            CommandType::Query {
                method: "debug/log_tail",
                params: json!({ "count": count }),
                format_key: "log-tail",
            }
        }
        "call" => {
            // Raw RPC: call <method> [json_params]
            if args.len() < 2 {
                return CommandType::Local(CliResult::Err(
                    "Usage: call <method> [params_json]".to_string(),
                ));
            }
            let method_str = args[1];
            let params_str = args.get(2..).map(|s| s.join(" ")).unwrap_or_default();
            let params = if params_str.is_empty() {
                json!({})
            } else {
                match serde_json::from_str(&params_str) {
                    Ok(v) => v,
                    Err(e) => {
                        return CommandType::Local(CliResult::Err(format!(
                            "Invalid JSON params: {e}"
                        )));
                    }
                }
            };
            // Box::leak rationale: The `call` command needs a dynamic method string,
            // but CommandType::Query expects &'static str for zero-copy efficiency.
            // We leak the string because:
            // 1. `call` is a rare debug/power-user command (<<1% of usage)
            // 2. Each leaked string is small (~20-50 bytes)
            // 3. Alternative (Cow<'static, str>) adds complexity for minimal benefit
            // 4. Session lifetime bounds the leak (server restart reclaims)
            // This is acceptable tech debt for v1. Future: enum wrapper or arena.
            CommandType::Query {
                method: Box::leak(method_str.to_string().into_boxed_str()),
                params,
                format_key: "call",
            }
        }

        _ => CommandType::Local(CliResult::Err(format!(
            "Unknown command: '{cmd}'. Type 'help' for commands."
        ))),
    }
}

/// Execute a fire-and-forget command.
///
/// Returns result immediately (success/error from send, not from server response).
pub async fn execute_fire_and_forget(writer: &mut RpcWriter, input: &str) -> CliResult {
    let args: Vec<&str> = input.split_whitespace().collect();
    let cmd = args.first().copied().unwrap_or("");

    match cmd {
        "keys" | "k" => {
            if args.len() < 2 {
                return CliResult::Err("Usage: keys <sequence>".to_string());
            }
            let keys = args[1..].join(" ");
            match writer
                .send_request("input/keys", json!({ "keys": keys }))
                .await
            {
                Ok(id) => CliResult::Ok(format!("Sent keys (id={id})")),
                Err(e) => CliResult::Err(format!("Error: {e}")),
            }
        }
        "resize" => {
            if args.len() < 3 {
                return CliResult::Err("Usage: resize <width> <height>".to_string());
            }
            let width: u16 = match args[1].parse() {
                Ok(w) => w,
                Err(_) => return CliResult::Err("Invalid width".to_string()),
            };
            let height: u16 = match args[2].parse() {
                Ok(h) => h,
                Err(_) => return CliResult::Err("Invalid height".to_string()),
            };
            match writer
                .send_request("editor/resize", json!({ "width": width, "height": height }))
                .await
            {
                Ok(id) => CliResult::Ok(format!("Resized to {width}x{height} (id={id})")),
                Err(e) => CliResult::Err(format!("Error: {e}")),
            }
        }
        "open" | "e" => {
            if args.len() < 2 {
                return CliResult::Err("Usage: open <path>".to_string());
            }
            let path = args[1..].join(" ");
            match writer
                .send_request("buffer/open_file", json!({ "path": path }))
                .await
            {
                Ok(id) => CliResult::Ok(format!("Opening '{path}' (id={id})")),
                Err(e) => CliResult::Err(format!("Error: {e}")),
            }
        }
        "kill" => match writer.send_request("server/kill", json!({})).await {
            Ok(_) => CliResult::Ok("Kill signal sent".to_string()),
            Err(e) => CliResult::Err(format!("Error: {e}")),
        },
        _ => CliResult::Err(format!("Not a fire-and-forget command: '{cmd}'")),
    }
}

const HELP_TEXT: &str = r"CLI Panel Commands:

State Queries:
  mode             Show current mode
  cursor           Show cursor position
  screen           Show screen dimensions
  buffers, ls      List buffers
  buffer [id]      Show buffer content
  modules          List loaded modules

Debug Queries:
  version          Server version
  uptime           Server uptime
  registers        Show register contents
  marks            Show mark contents
  log-level [lvl]  Get/set log level
  log-tail [n]     Show recent log entries (default: 20)

Raw RPC:
  call <method> [params]   Execute raw RPC method

Write Commands:
  keys <seq>       Inject key sequence (e.g., keys iHello<Esc>)
  k <seq>          Alias for keys
  resize <w> <h>   Resize editor
  open <path>      Open file (alias: e)
  kill             Kill server

Panel Control:
  help, ?          Show this help
  clear            Clear history
  quit, q          Close panel (or use Esc/<C-b>;)

Panel Keys:
  Enter            Execute command
  Up/Down          History navigation
  PageUp/PageDown  Scroll history
  Shift+G          Scroll to bottom
  Esc              Close panel
  <C-b>;           Toggle panel
";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_help_command() {
        assert!(!HELP_TEXT.is_empty());
        assert!(HELP_TEXT.contains("keys"));
        assert!(HELP_TEXT.contains("help"));
        assert!(HELP_TEXT.contains("mode"));
        assert!(HELP_TEXT.contains("cursor"));
    }

    #[test]
    fn test_classify_local_commands() {
        // Help
        assert!(matches!(classify_command("help"), CommandType::Local(_)));
        assert!(matches!(classify_command("?"), CommandType::Local(_)));

        // Quit
        assert!(matches!(classify_command("quit"), CommandType::Local(_)));
        assert!(matches!(classify_command("q"), CommandType::Local(_)));

        // Clear
        assert!(matches!(classify_command("clear"), CommandType::Local(_)));

        // Empty
        assert!(matches!(classify_command(""), CommandType::Local(_)));

        // Unknown
        assert!(matches!(classify_command("unknown"), CommandType::Local(CliResult::Err(_))));
    }

    #[test]
    fn test_classify_fire_and_forget_commands() {
        assert!(matches!(classify_command("keys hello"), CommandType::FireAndForget));
        assert!(matches!(classify_command("k hello"), CommandType::FireAndForget));
        assert!(matches!(classify_command("resize 80 24"), CommandType::FireAndForget));
        assert!(matches!(classify_command("open file.txt"), CommandType::FireAndForget));
        assert!(matches!(classify_command("kill"), CommandType::FireAndForget));
    }

    #[test]
    fn test_classify_query_commands() {
        // Mode
        if let CommandType::Query {
            method, format_key, ..
        } = classify_command("mode")
        {
            assert_eq!(method, "state/mode");
            assert_eq!(format_key, "mode");
        } else {
            panic!("Expected Query variant for mode");
        }

        // Cursor
        if let CommandType::Query {
            method, format_key, ..
        } = classify_command("cursor")
        {
            assert_eq!(method, "state/cursor");
            assert_eq!(format_key, "cursor");
        } else {
            panic!("Expected Query variant for cursor");
        }

        // Buffers
        if let CommandType::Query {
            method, format_key, ..
        } = classify_command("buffers")
        {
            assert_eq!(method, "buffer/list");
            assert_eq!(format_key, "buffers");
        } else {
            panic!("Expected Query variant for buffers");
        }

        // ls alias
        if let CommandType::Query {
            method, format_key, ..
        } = classify_command("ls")
        {
            assert_eq!(method, "buffer/list");
            assert_eq!(format_key, "buffers");
        } else {
            panic!("Expected Query variant for ls");
        }

        // Version
        if let CommandType::Query {
            method, format_key, ..
        } = classify_command("version")
        {
            assert_eq!(method, "debug/version");
            assert_eq!(format_key, "version");
        } else {
            panic!("Expected Query variant for version");
        }
    }

    #[test]
    fn test_classify_buffer_with_id() {
        if let CommandType::Query { method, params, .. } = classify_command("buffer 42") {
            assert_eq!(method, "buffer/get_content");
            assert_eq!(params["buffer_id"], 42);
        } else {
            panic!("Expected Query variant for buffer with id");
        }
    }

    #[test]
    fn test_classify_log_tail_with_count() {
        if let CommandType::Query { method, params, .. } = classify_command("log-tail 100") {
            assert_eq!(method, "debug/log_tail");
            assert_eq!(params["count"], 100);
        } else {
            panic!("Expected Query variant for log-tail with count");
        }
    }

    #[test]
    fn test_classify_call_command() {
        // Valid call
        if let CommandType::Query { method, params, .. } =
            classify_command(r#"call test/method {"key": "value"}"#)
        {
            assert_eq!(method, "test/method");
            assert_eq!(params["key"], "value");
        } else {
            panic!("Expected Query variant for call");
        }

        // Missing method
        assert!(matches!(classify_command("call"), CommandType::Local(CliResult::Err(_))));

        // Invalid JSON
        assert!(matches!(
            classify_command("call test/method {invalid}"),
            CommandType::Local(CliResult::Err(_))
        ));
    }

    #[test]
    fn test_classify_whitespace_handling() {
        // Extra whitespace
        assert!(matches!(classify_command("  mode  "), CommandType::Query { .. }));

        // Multiple spaces between args
        assert!(matches!(classify_command("buffer    42"), CommandType::Query { .. }));
    }
}
