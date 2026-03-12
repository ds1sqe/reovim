#![cfg_attr(coverage_nightly, allow(unused_features))]
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
        /// Keys in vim notation (e.g., `iHello<Esc>`).
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

    /// Capture screen content.
    ///
    /// For text formats (`plain_text`, `raw_ansi`, `cell_grid`): captures via gRPC relay
    /// from a connected TUI client (requires `--client`).
    ///
    /// For visual formats (`png`, `html`): captures via Playwright headless browser
    /// running the real web client (requires `--web-url`).
    Capture {
        /// Target client ID (required for text capture, ignored for web capture).
        #[arg(long, short)]
        client: Option<u64>,

        /// Capture format: `raw_ansi`, `plain_text`, `cell_grid`, `png`, `html`.
        #[arg(long, short = 'f', default_value = "raw_ansi")]
        capture_format: String,

        /// Web client URL for visual capture (required for png/html formats).
        #[arg(long)]
        web_url: Option<String>,

        /// Viewport width in pixels (web capture only).
        #[arg(long, default_value = "1920")]
        width: u32,

        /// Viewport height in pixels (web capture only).
        #[arg(long, default_value = "1080")]
        height: u32,

        /// Device pixel ratio (web capture only).
        #[arg(long, default_value = "1")]
        dpr: u32,

        /// Output file path (web capture only; stdout if omitted).
        #[arg(long, short)]
        output: Option<String>,
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
                web_url,
                width,
                height,
                dpr,
                output,
            } => {
                let address = &self.grpc;
                commands::capture(
                    &mut client,
                    *client_id,
                    capture_format,
                    web_url.as_deref(),
                    address,
                    *width,
                    *height,
                    *dpr,
                    output.as_deref(),
                    self.format,
                )
                .await
            }
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
#[path = "lib_tests.rs"]
mod tests;
