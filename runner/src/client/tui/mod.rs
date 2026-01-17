//! TUI client for interactive terminal editing.
//!
//! Connects to a reovim server and provides a terminal user interface.

use std::path::PathBuf;

use clap::Args;

use crate::client::common::ConnectionConfig;

pub mod app;
pub mod input;
pub mod log_buffer;
pub mod log_panel;
pub mod log_render;
pub mod render;

pub use {
    app::TuiApp,
    input::InputHandler,
    log_buffer::{LevelColor, TuiLogBuffer, TuiLogEntry},
    log_panel::LogPanelState,
    log_render::{format_entry, render_panel},
    render::Renderer,
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

    /// Connect to server via Unix socket (legacy, use -S instead).
    #[cfg(unix)]
    #[arg(long, value_name = "PATH", hide = true)]
    pub socket: Option<PathBuf>,
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
        // Handle legacy --socket flag (maps to -S)
        #[cfg(unix)]
        let socket_path = self.socket_path.or(self.socket);
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
}
