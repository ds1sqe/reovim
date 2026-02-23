#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! Reovim CLI Client - gRPC v2 command-line interface.
//!
//! This crate provides a CLI client for interacting with reovim servers
//! using the gRPC v2 protocol.
//!
//! # Example
//!
//! ```ignore
//! use reovim_client_cli::{CliArgs, CliAction};
//!
//! #[tokio::main]
//! async fn main() {
//!     let args = CliArgs::parse();
//!     let result = args.execute().await;
//!     match result {
//!         Ok(output) => println!("{}", output),
//!         Err(e) => eprintln!("Error: {}", e),
//!     }
//! }
//! ```
//!
//! # Commands
//!
//! - `keys <KEYS>` - Send keys in vim notation
//! - `mode` - Get current editor mode
//! - `cursor` - Get cursor position
//! - `buffers` - List open buffers
//! - `buffer [ID]` - Get buffer content
//! - `ping` - Health check
//! - `version` - Get server version
//!
//! # Protocol
//!
//! This CLI uses **gRPC v2** transport, not JSON-RPC v1.
//! Connect to a server started with `--grpc <PORT>`.

mod client;
pub mod commands;

use clap::{Parser, Subcommand};
pub use client::{GrpcClient, GrpcClientError};

/// CLI arguments for the gRPC v2 CLI client.
#[derive(Debug, Parser)]
#[command(name = "reovim-cli")]
#[command(about = "Reovim CLI client (gRPC v2)", long_about = None)]
pub struct CliArgs {
    /// gRPC server address (host:port).
    #[arg(long, default_value = "127.0.0.1:12540")]
    pub grpc: String,

    /// Output format.
    #[arg(long, short, value_enum, default_value = "plain")]
    pub format: OutputFormat,

    /// Command to execute.
    #[command(subcommand)]
    pub command: CliCommand,
}

/// Output format for CLI results.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum OutputFormat {
    /// Plain text output.
    Plain,
    /// JSON output.
    Json,
}

/// CLI commands.
#[derive(Debug, Subcommand)]
pub enum CliCommand {
    /// Send keys to a specific client.
    Keys {
        /// Keys in vim notation (e.g., "iHello<Esc>").
        keys: String,

        /// Target client ID to send keys to (required).
        #[arg(long, short)]
        client: u64,
    },

    /// Get a specific client's editor mode.
    Mode {
        /// Target client ID to query mode from (required).
        #[arg(long, short)]
        client: u64,
    },

    /// Get a specific client's cursor position.
    Cursor {
        /// Target client ID to query cursor from (required).
        #[arg(long, short)]
        client: u64,
    },

    /// List open buffers.
    Buffers,

    /// Get buffer content.
    Buffer {
        /// Buffer ID (uses active buffer if not specified).
        #[arg(long)]
        id: Option<u64>,
    },

    /// Get register contents.
    ///
    /// Without arguments, lists all non-empty registers.
    /// With a register name, shows that register's content.
    Registers {
        /// Register name (e.g., "a", "\"", "0").
        name: Option<String>,
    },

    /// Capture TUI screen content.
    ///
    /// Requests a screen capture from a specific TUI client.
    /// Requires a headless TUI to be connected to the server.
    Capture {
        /// Target client ID to capture from (required).
        #[arg(long, short)]
        client: u64,

        /// Capture format: `plain_text`, `raw_ansi` (default), `cell_grid`.
        #[arg(long, short = 'f', default_value = "raw_ansi")]
        capture_format: String,
    },

    /// Ping the server (health check).
    Ping,

    /// Get server version and info.
    Version,

    /// List connected clients (read-only debug query).
    Clients,

    /// Query extension state (e.g., which-key, cmdline).
    ExtensionState {
        /// Extension kind to query (e.g., "whichkey", "cmdline").
        kind: String,

        /// Target client ID.
        #[arg(long, short)]
        client: u64,
    },

    /// List registered extensions.
    Extensions,
}

impl CliArgs {
    /// Execute the CLI command.
    ///
    /// # Errors
    ///
    /// Returns an error if the gRPC connection fails or the command fails.
    #[cfg_attr(coverage_nightly, coverage(off))]
    pub async fn execute(&self) -> Result<String, GrpcClientError> {
        let mut client = GrpcClient::connect(&self.grpc).await?;

        match &self.command {
            CliCommand::Keys {
                keys,
                client: target,
            } => commands::keys(&mut client, keys, *target, self.format).await,
            CliCommand::Mode { client: target } => {
                commands::mode(&mut client, *target, self.format).await
            }
            CliCommand::Cursor { client: target } => {
                commands::cursor(&mut client, *target, self.format).await
            }
            CliCommand::Buffers => commands::buffers(&mut client, self.format).await,
            CliCommand::Buffer { id } => commands::buffer(&mut client, *id, self.format).await,
            CliCommand::Registers { name } => {
                commands::registers(&mut client, name.clone(), self.format).await
            }
            CliCommand::Capture {
                client: client_id,
                capture_format,
            } => commands::capture(&mut client, *client_id, capture_format, self.format).await,
            CliCommand::Ping => commands::ping(&mut client, self.format).await,
            CliCommand::Version => commands::version(&mut client, self.format).await,
            CliCommand::Clients => commands::clients(&mut client, self.format).await,
            CliCommand::ExtensionState {
                kind,
                client: target,
            } => commands::extension_state(&mut client, kind, *target, self.format).await,
            CliCommand::Extensions => commands::extensions(&mut client, self.format).await,
        }
    }
}

#[cfg(test)]
mod tests {
    use {super::*, clap::Parser};

    #[test]
    fn test_cli_args_parse_keys() {
        let args = CliArgs::parse_from(["reovim-cli", "keys", "--client", "1", "iHello"]);
        match &args.command {
            CliCommand::Keys { keys, client } => {
                assert_eq!(keys, "iHello");
                assert_eq!(*client, 1);
            }
            _ => panic!("Expected Keys command"),
        }
    }

    #[test]
    fn test_cli_args_parse_mode() {
        let args = CliArgs::parse_from(["reovim-cli", "mode", "--client", "1"]);
        match &args.command {
            CliCommand::Mode { client } => {
                assert_eq!(*client, 1);
            }
            _ => panic!("Expected Mode command"),
        }
    }

    #[test]
    fn test_cli_args_parse_cursor() {
        let args = CliArgs::parse_from(["reovim-cli", "cursor", "--client", "2"]);
        match &args.command {
            CliCommand::Cursor { client } => {
                assert_eq!(*client, 2);
            }
            _ => panic!("Expected Cursor command"),
        }
    }

    #[test]
    fn test_cli_args_custom_address() {
        let args = CliArgs::parse_from(["reovim-cli", "--grpc", "localhost:50051", "ping"]);
        assert_eq!(args.grpc, "localhost:50051");
    }

    #[test]
    fn test_cli_args_json_format() {
        let args = CliArgs::parse_from(["reovim-cli", "--format", "json", "version"]);
        assert_eq!(args.format, OutputFormat::Json);
    }

    #[test]
    fn test_cli_args_clients() {
        let args = CliArgs::parse_from(["reovim-cli", "clients"]);
        assert!(matches!(args.command, CliCommand::Clients));
    }

    #[test]
    fn test_cli_args_extension_state() {
        let args =
            CliArgs::parse_from(["reovim-cli", "extension-state", "whichkey", "--client", "1"]);
        match &args.command {
            CliCommand::ExtensionState { kind, client } => {
                assert_eq!(kind, "whichkey");
                assert_eq!(*client, 1);
            }
            _ => panic!("Expected ExtensionState command"),
        }
    }

    #[test]
    fn test_cli_args_extensions() {
        let args = CliArgs::parse_from(["reovim-cli", "extensions"]);
        assert!(matches!(args.command, CliCommand::Extensions));
    }
}
