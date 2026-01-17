//! CLI client for scripting and automation.
//!
//! Provides command-line interface for interacting with a reovim server.

use std::path::PathBuf;

use clap::{Args, Subcommand};

use crate::client::common::ConnectionConfig;

pub mod commands;
pub mod output;
pub mod repl;

pub use {commands::*, output::OutputFormat, repl::run_repl};

/// CLI mode arguments.
///
/// These arguments configure CLI connection and behavior.
#[derive(Args, Debug, Clone)]
pub struct CliArgs {
    /// Connect to server via TCP (e.g., 127.0.0.1:12521).
    #[arg(long, value_name = "ADDR")]
    pub tcp: Option<String>,

    /// Connect to server via Unix socket.
    #[cfg(unix)]
    #[arg(long, value_name = "PATH")]
    pub socket: Option<PathBuf>,

    /// Start interactive REPL mode.
    #[arg(short = 'i', long)]
    pub repl: bool,

    /// Output format: plain, json.
    #[arg(short, long, default_value = "plain", value_name = "FORMAT")]
    pub format: String,

    /// CLI action to execute.
    #[command(subcommand)]
    pub action: Option<CliAction>,
}

impl CliArgs {
    /// Convert arguments to `ConnectionConfig`.
    #[must_use]
    pub fn into_config(&self) -> ConnectionConfig {
        #[cfg(unix)]
        if let Some(ref path) = self.socket {
            return ConnectionConfig::unix_socket(path);
        }

        self.tcp
            .as_ref()
            .map_or_else(ConnectionConfig::auto_discover, |addr| {
                ConnectionConfig::tcp_from_addr(addr)
            })
    }
}

/// CLI actions (subcommands of `reovim cli`).
#[derive(Subcommand, Clone, Debug)]
pub enum CliAction {
    /// Inject key sequence.
    Keys {
        /// Key sequence to inject (e.g., `iHello<Esc>`).
        keys: String,
    },
    /// Get current mode.
    Mode,
    /// Get cursor position.
    Cursor,
    /// Get screen dimensions.
    Screen,
    /// Get screen content.
    Content {
        /// Output format: `plain_text`, `raw_ansi`, `cell_grid`.
        format: Option<String>,
    },
    /// List buffers.
    Buffers,
    /// Get buffer content.
    Buffer {
        /// Buffer ID (uses active buffer if not specified).
        id: Option<u64>,
    },
    /// Open file.
    Open {
        /// Path to the file to open.
        path: String,
    },
    /// Resize editor.
    Resize {
        /// New width in columns.
        width: u64,
        /// New height in rows.
        height: u64,
    },
    /// List loaded modules.
    Modules,
    /// Load module.
    Load {
        /// Path to the module file.
        path: String,
    },
    /// Unload module.
    Unload {
        /// Module ID to unload.
        id: String,
    },
    /// Reload module.
    Reload {
        /// Module ID to reload.
        id: String,
    },
    /// Force kill server.
    Kill,
    /// Send raw JSON-RPC request.
    Raw {
        /// JSON string to send.
        json: String,
    },
    /// List running servers (no connection needed).
    List,
    /// Get current selection (visual mode).
    Selection,
    /// Gracefully quit the editor.
    Quit,
    /// Set buffer content.
    #[command(name = "set-content")]
    SetContent {
        /// The content to set.
        content: String,
    },

    // Debug commands
    /// Get server version information.
    Version,
    /// Get server uptime.
    Uptime,
    /// Get kernel state summary.
    KernelState,
    /// Get register contents.
    Registers,
    /// Get mark contents.
    Marks,
    /// Get mode stack.
    ModeStack,
    /// Get performance metrics.
    Metrics,
    /// Get handler statistics.
    Handlers,
    /// Get or set log level.
    LogLevel {
        /// New log level to set (omit to get current).
        level: Option<String>,
    },
    /// Get recent log entries.
    LogTail {
        /// Number of entries to return (default: 50).
        #[arg(short, long, default_value = "50")]
        count: usize,

        /// Filter by minimum log level (trace, debug, info, warn, error).
        #[arg(short = 'l', long)]
        level: Option<String>,

        /// Filter by target module (substring match).
        #[arg(short = 't', long)]
        target: Option<String>,

        /// Filter by message content (case-insensitive).
        #[arg(short = 'g', long)]
        grep: Option<String>,

        /// Follow mode: stream new entries as they arrive (like tail -f).
        #[arg(short = 'f', long)]
        follow: bool,
    },
    /// Get full debug snapshot (JSON).
    Snapshot,
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use super::*;

    /// Test CLI for parsing subcommands
    #[derive(Parser)]
    struct TestCli {
        #[command(subcommand)]
        action: CliAction,
    }

    #[test]
    fn test_cli_action_selection_parse() {
        let cli = TestCli::try_parse_from(["test", "selection"]).unwrap();
        assert!(matches!(cli.action, CliAction::Selection));
    }

    #[test]
    fn test_cli_action_quit_parse() {
        let cli = TestCli::try_parse_from(["test", "quit"]).unwrap();
        assert!(matches!(cli.action, CliAction::Quit));
    }

    #[test]
    fn test_cli_action_set_content_parse() {
        let cli = TestCli::try_parse_from(["test", "set-content", "Hello World"]).unwrap();
        match cli.action {
            CliAction::SetContent { content } => {
                assert_eq!(content, "Hello World");
            }
            _ => panic!("Expected SetContent variant"),
        }
    }

    #[test]
    fn test_cli_action_set_content_empty() {
        let cli = TestCli::try_parse_from(["test", "set-content", ""]).unwrap();
        match cli.action {
            CliAction::SetContent { content } => {
                assert!(content.is_empty());
            }
            _ => panic!("Expected SetContent variant"),
        }
    }
}
