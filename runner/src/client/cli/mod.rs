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
}
