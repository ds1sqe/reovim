//! TUI client for interactive terminal editing.
//!
//! Connects to a reovim server and provides a terminal user interface.

use std::path::PathBuf;

use clap::Args;

use crate::client::common::ConnectionConfig;

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

pub mod app;
pub mod cli_executor;
pub mod cli_panel;
pub mod cli_render;
pub mod headless;
pub mod input;
pub mod log_buffer;
pub mod log_panel;
pub mod log_render;
pub mod render;
pub mod render_core;

pub use {
    app::TuiApp,
    cli_panel::{CliHistoryEntry, CliPanelState, CliResult},
    headless::HeadlessClient,
    input::InputHandler,
    log_buffer::{LevelColor, TuiLogBuffer, TuiLogEntry},
    log_panel::LogPanelState,
    log_render::{format_entry, render_panel},
    render::Renderer,
    render_core::{RenderState, build_frame_content},
};

/// TUI mode CLI arguments.
///
/// These arguments configure how the TUI connects to a server.
///
/// Flag precedence (highest to lowest):
/// 1. `--tcp` - explicit TCP connection
/// 2. `-S`/`--socket-path` - explicit socket path
/// 3. `-L`/`--instance` - named instance lookup
/// 4. Default: auto-discover or `-L default`
#[derive(Args, Debug, Clone)]
pub struct TuiArgs {
    /// Connect to named instance (default: "default").
    ///
    /// Looks up the instance in the registry and connects using its
    /// registered transport (TCP or local socket).
    #[arg(short = 'L', long = "instance", value_name = "NAME")]
    pub instance: Option<String>,

    /// Connect via explicit socket/pipe path.
    ///
    /// Takes precedence over `-L` but not over `--tcp`.
    #[arg(short = 'S', long = "socket-path", value_name = "PATH")]
    pub socket_path: Option<PathBuf>,

    /// Connect to server via TCP (e.g., 127.0.0.1:12521).
    ///
    /// Takes highest precedence - bypasses instance registry entirely.
    #[arg(long, value_name = "ADDR")]
    pub tcp: Option<String>,

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

    /// Run in headless mode (no TTY required).
    ///
    /// Connects to the server and responds to capture requests only.
    /// Useful for CI/testing and programmatic frame capture.
    #[arg(long)]
    pub headless: bool,
}

impl TuiArgs {
    /// Convert arguments to `ConnectionConfig`.
    ///
    /// # Panics
    ///
    /// Panics if instance lookup fails. For fallible resolution, use
    /// `ConnectionConfig::from_flags` directly.
    #[must_use]
    pub fn into_config(self) -> ConnectionConfig {
        #[cfg(unix)]
        let socket_path = self.socket_path;
        #[cfg(not(unix))]
        let socket_path = self.socket_path;

        // If instance flag is set, use registry lookup
        if self.tcp.is_some() || socket_path.is_some() || self.instance.is_some() {
            match ConnectionConfig::from_flags(
                self.tcp.as_deref(),
                socket_path.as_deref(),
                self.instance.as_deref(),
            ) {
                Ok(config) => return config,
                Err(e) => {
                    eprintln!("Error: {e}");
                    std::process::exit(1);
                }
            }
        }

        // No flags: auto-discover
        ConnectionConfig::auto_discover()
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
