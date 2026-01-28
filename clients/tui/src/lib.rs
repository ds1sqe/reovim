//! Reovim TUI Client - terminal user interface (gRPC v2 only).
//!
//! This crate provides the terminal user interface for connecting
//! to reovim servers via gRPC v2 protocol.
//!
//! - **`TuiAppV2`**: Main event loop with gRPC streaming
//! - **Rendering**: Client-side rendering via local frame buffer
//! - **Input**: Keyboard input handling
//!
//! # Architecture (Epic #465 Phase 10)
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────┐
//! │  lib/clients/tui/                  (THIS CRATE)             │
//! │    TuiAppV2, TuiGrpcClient, Renderer                        │
//! ├─────────────────────────────────────────────────────────────┤
//! │  lib/protocol/                     (gRPC v2 types)          │
//! └─────────────────────────────────────────────────────────────┘
//! ```
//!
//! NOTE: v1 JSON-RPC code has been archived to `archive/post_kernel/lib/clients/tui/`.

use std::path::PathBuf;

use clap::Args;

/// Debug configuration for TUI.
///
/// Created when `--debug` flag is passed. Controls debug output paths
/// and enables debug features like statusline and frame capture.
#[derive(Debug, Clone)]
pub struct TuiDebugConfig {
    /// Base directory for debug output.
    pub log_dir: PathBuf,
    /// Session name (used in filenames).
    pub name: String,
    /// Session start time in `YYYYMMDDHHmmss` format.
    pub start_time: String,
}

impl TuiDebugConfig {
    /// Create a new debug config with the given parameters.
    ///
    /// # Arguments
    ///
    /// * `log_dir` - Base directory for debug output
    /// * `name` - Session name for filenames
    #[must_use]
    pub fn new(log_dir: PathBuf, name: String) -> Self {
        let start_time = chrono::Local::now().format("%Y%m%d%H%M%S").to_string();
        Self {
            log_dir,
            name,
            start_time,
        }
    }

    /// Get the path for a frame capture file.
    ///
    /// Format: `{log_dir}/frame-buffer/{name}-{timestamp}.frame`
    #[must_use]
    pub fn frame_capture_path(&self, timestamp: &str) -> PathBuf {
        self.log_dir
            .join("frame-buffer")
            .join(format!("{}-{}.frame", self.name, timestamp))
    }

    /// Get the path for the session log file.
    ///
    /// Format: `{log_dir}/{name}_{start_time}.log`
    #[must_use]
    pub fn session_log_path(&self) -> PathBuf {
        self.log_dir
            .join(format!("{}_{}.log", self.name, self.start_time))
    }
}

// Module declarations - gRPC v2 only
pub mod cli_panel;
pub mod cli_render;
pub mod input;
pub mod log_buffer;
pub mod log_panel;
pub mod log_render;
pub mod render;
pub mod render_core;

// gRPC v2 client
#[cfg(feature = "grpc")]
pub mod grpc_client;

// gRPC v2 TUI app
#[cfg(feature = "grpc")]
pub mod app_v2;

// gRPC v2 headless TUI app
#[cfg(feature = "grpc")]
pub mod app_v2_headless;

pub use {
    cli_panel::{CliHistoryEntry, CliPanelState, CliResult},
    input::InputHandler,
    log_buffer::{LevelColor, TuiLogBuffer, TuiLogEntry},
    log_panel::LogPanelState,
    log_render::{format_entry, render_panel},
    render::Renderer,
    render_core::{RenderState, build_frame_content},
};

// gRPC v2 client exports
#[cfg(feature = "grpc")]
pub use grpc_client::{TuiGrpcClient, TuiGrpcError};

// gRPC v2 TUI app exports
#[cfg(feature = "grpc")]
pub use app_v2::{TuiAppV2, TuiAppV2Error};

// gRPC v2 headless TUI app exports
#[cfg(feature = "grpc")]
pub use app_v2_headless::{FrameMetadata, HeadlessError, TuiAppV2Headless};

/// TUI mode CLI arguments.
///
/// These arguments configure how the TUI connects to a gRPC server.
#[derive(Args, Debug, Clone)]
pub struct TuiArgs {
    /// gRPC server address (e.g., "127.0.0.1:50051").
    ///
    /// Default: 127.0.0.1:50051
    #[arg(long = "grpc", value_name = "ADDR")]
    pub grpc_addr: Option<String>,

    /// Run in headless mode (no terminal, for testing/scripting).
    ///
    /// Headless mode connects to the server and processes notifications
    /// without rendering to a terminal. Use with `reovim cli capture`
    /// to retrieve frame content.
    #[arg(long)]
    pub headless: bool,

    /// Enable debug mode (statusline + frame capture).
    ///
    /// Shows a debug statusline at the bottom of the screen and
    /// captures frame buffers periodically to files.
    #[arg(long)]
    pub debug: bool,

    /// Debug log directory.
    ///
    /// Default: ~/.local/share/reovim/logs/tui/
    #[arg(long, value_name = "DIR")]
    pub debug_dir: Option<PathBuf>,

    /// Debug session name.
    ///
    /// Used in log filenames. Default: "default"
    #[arg(long, value_name = "NAME")]
    pub debug_name: Option<String>,
}

impl TuiArgs {
    /// Get the gRPC server address.
    ///
    /// Returns the specified address or the default `127.0.0.1:50051`.
    #[must_use]
    pub fn grpc_address(&self) -> String {
        self.grpc_addr
            .clone()
            .unwrap_or_else(|| "127.0.0.1:50051".to_string())
    }

    /// Create debug configuration if debug mode is enabled.
    ///
    /// Returns `None` if `--debug` flag is not set.
    #[must_use]
    pub fn into_debug_config(&self) -> Option<TuiDebugConfig> {
        if !self.debug {
            return None;
        }

        // Resolve log directory
        let log_dir = self.debug_dir.clone().unwrap_or_else(|| {
            reovim_arch::dirs::data_local_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join("reovim")
                .join("logs")
                .join("tui")
        });

        // Resolve session name
        let name = self
            .debug_name
            .clone()
            .unwrap_or_else(|| "default".to_string());

        Some(TuiDebugConfig::new(log_dir, name))
    }
}
