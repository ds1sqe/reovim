//! CLI command execution for embedded REPL.
//!
//! Adapts CLI commands for use with `RpcWriter` (async, fire-and-forget).
//! Query commands that need responses are not supported - users should
//! use `reovim cli -i` for full interactive mode.

use serde_json::json;

use crate::client::common::RpcWriter;

use super::cli_panel::CliResult;

/// Execute a CLI command and return formatted result.
///
/// Commands are parsed from whitespace-separated input.
/// Uses fire-and-forget RPC for commands that don't need responses.
///
/// # Arguments
///
/// * `writer` - RPC writer for sending requests
/// * `input` - Raw command input string
///
/// # Returns
///
/// Result indicating success or error with message.
pub async fn execute_command(writer: &mut RpcWriter, input: &str) -> CliResult {
    let args: Vec<&str> = input.split_whitespace().collect();
    let cmd = args.first().copied().unwrap_or("");

    match cmd {
        "help" | "?" => CliResult::Ok(HELP_TEXT.to_string()),

        // Commands that modify state (fire-and-forget OK)
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

        // Commands that need responses - not supported in embedded panel
        "mode" | "cursor" | "screen" | "buffers" | "ls" | "buffer" | "modules" | "version"
        | "uptime" | "registers" | "marks" | "content" | "selection" => CliResult::Err(format!(
            "'{cmd}' requires response - use 'reovim cli -i' for interactive mode"
        )),

        // Panel control commands
        "quit" | "exit" | "q" => CliResult::Ok("Use Esc or Ctrl+; to close panel".to_string()),

        "clear" => CliResult::Ok("__CLEAR__".to_string()),

        "" => CliResult::Ok(String::new()),

        _ => CliResult::Err(format!("Unknown command: '{cmd}'. Type 'help' for commands.")),
    }
}

const HELP_TEXT: &str = r"CLI Panel Commands:

Write Commands (fire-and-forget):
  keys <seq>       Inject key sequence (e.g., keys iHello<Esc>)
  k <seq>          Alias for keys
  resize <w> <h>   Resize editor
  open <path>      Open file (alias: e)
  kill             Kill server

Panel Control:
  help, ?          Show this help
  clear            Clear history
  quit, q          Close panel (or use Esc/<C-b>;)

Query commands (mode, cursor, buffers, etc.) require 'reovim cli -i'

Panel Keys:
  Enter            Execute command
  Up/Down          History navigation
  Esc              Close panel
  <C-b>;           Toggle panel
";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_help_command() {
        // Can't test async easily, just verify help text exists
        assert!(!HELP_TEXT.is_empty());
        assert!(HELP_TEXT.contains("keys"));
        assert!(HELP_TEXT.contains("help"));
    }
}
