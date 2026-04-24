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
//! - `module <SUBCOMMAND>` - Manage installed third-party modules
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

    /// Manage installed modules (install, remove, update, list, info, check).
    Module {
        /// Module management subcommand.
        #[command(subcommand)]
        subcommand: ModuleSubcommand,
    },

    /// Driver-owned debug surface verbs (#770). All payloads are
    /// opaque to the CLI; the driver owns the wire vocabulary.
    Debug {
        /// Debug-surface subcommand.
        #[command(subcommand)]
        subcommand: DebugSubcommand,
    },
}

/// Driver-owned debug-surface subcommands (#770 Phase 2).
#[derive(Debug, Subcommand)]
pub enum DebugSubcommand {
    /// Probe a registered driver: print its metadata and schema lists.
    Probe {
        /// Driver name (required; no-arg listing deferred to #771 pkg).
        #[arg(long)]
        driver: String,
    },

    /// Observe a stream of frames from a driver schema. Runs until
    /// the driver emits EOS, the user hits Ctrl-C, or `--count N`
    /// frames have been printed.
    Observe {
        /// Driver name.
        #[arg(long)]
        driver: String,

        /// Observe-schema name as declared in the driver probe.
        #[arg(long)]
        schema: String,

        /// Stop after printing N frames (sends `ObserveStop` before
        /// closing the client stream).
        #[arg(long)]
        count: Option<u32>,
    },

    /// Drive a driver command: send opaque bytes, print the response.
    Drive {
        /// Driver name.
        #[arg(long)]
        driver: String,

        /// Drive-schema name as declared in the driver probe.
        #[arg(long)]
        schema: String,

        /// Input bytes. Prefix with `@` to read from a file path,
        /// otherwise the string is passed through as UTF-8 bytes.
        #[arg(long)]
        input: String,
    },
}

/// Module management subcommands.
#[derive(Debug, Subcommand)]
pub enum ModuleSubcommand {
    /// Install a module from a git URL or local path.
    Install {
        /// Git URL (https://... or git@...) or local path.
        source: String,

        /// Pin to a specific git branch, tag, or commit.
        #[arg(long)]
        rev: Option<String>,
    },

    /// Remove an installed module.
    Remove {
        /// Module ID.
        id: String,
    },

    /// Update an installed module (or all if no ID given).
    Update {
        /// Module ID to update. Omit to update all installed modules.
        id: Option<String>,
    },

    /// List installed modules with optional loaded-status from a running server.
    List {
        /// Include loaded status from a running server.
        #[arg(long)]
        loaded: bool,
    },

    /// Show details for an installed module.
    Info {
        /// Module ID.
        id: String,
    },

    /// Check integrity of all installed modules.
    Check,
}

impl CliCommand {
    const fn requires_grpc(&self) -> bool {
        match self {
            Self::Module {
                subcommand: ModuleSubcommand::List { loaded },
            } => *loaded,
            Self::Module { .. } => false,
            _ => true,
        }
    }
}

impl CliArgs {
    /// Execute the CLI command.
    ///
    /// # Errors
    ///
    /// Returns an error if the gRPC connection fails or the command fails.
    #[cfg_attr(coverage_nightly, coverage(off))]
    pub async fn execute(&self) -> Result<String, GrpcClientError> {
        let mut client = if self.command.requires_grpc() {
            Some(GrpcClient::connect(&self.grpc).await?)
        } else {
            None
        };

        match &self.command {
            CliCommand::Keys {
                keys,
                client: target,
            } => {
                let client = connected_client(&mut client, "keys")?;
                commands::keys(client, keys, *target, self.format).await
            }
            CliCommand::Mode { client: target } => {
                let client = connected_client(&mut client, "mode")?;
                commands::mode(client, *target, self.format).await
            }
            CliCommand::Cursor { client: target } => {
                let client = connected_client(&mut client, "cursor")?;
                commands::cursor(client, *target, self.format).await
            }
            CliCommand::Buffers => {
                let client = connected_client(&mut client, "buffers")?;
                commands::buffers(client, self.format).await
            }
            CliCommand::Buffer { id } => {
                let client = connected_client(&mut client, "buffer")?;
                commands::buffer(client, *id, self.format).await
            }
            CliCommand::Registers { name } => {
                let client = connected_client(&mut client, "registers")?;
                commands::registers(client, name.clone(), self.format).await
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
                let client = connected_client(&mut client, "capture")?;
                let address = &self.grpc;
                commands::capture(
                    client,
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
            CliCommand::Ping => {
                let client = connected_client(&mut client, "ping")?;
                commands::ping(client, self.format).await
            }
            CliCommand::Version => {
                let client = connected_client(&mut client, "version")?;
                commands::version(client, self.format).await
            }
            CliCommand::Clients => {
                let client = connected_client(&mut client, "clients")?;
                commands::clients(client, self.format).await
            }
            CliCommand::ExtensionState {
                kind,
                client: target,
            } => {
                let client = connected_client(&mut client, "extension-state")?;
                commands::extension_state(client, kind, *target, self.format).await
            }
            CliCommand::Extensions => {
                let client = connected_client(&mut client, "extensions")?;
                commands::extensions(client, self.format).await
            }
            CliCommand::Module { subcommand } => {
                commands::module(client.as_mut(), subcommand, self.format).await
            }
            CliCommand::Debug { subcommand } => {
                let client = connected_client(&mut client, "debug")?;
                commands::debug::dispatch(client, subcommand, self.format).await
            }
        }
    }
}

#[allow(clippy::result_large_err)]
fn connected_client<'a>(
    client: &'a mut Option<GrpcClient>,
    command: &str,
) -> Result<&'a mut GrpcClient, GrpcClientError> {
    client.as_mut().ok_or_else(|| {
        GrpcClientError::ConnectionFailed(format!(
            "internal error: missing gRPC client for '{command}' command"
        ))
    })
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
