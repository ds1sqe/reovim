//! TUI client for interactive terminal editing.
//!
//! Connects to a reovim server and provides a terminal user interface.

use std::path::PathBuf;

use clap::Args;

use crate::client::common::ConnectionConfig;

pub mod app;
pub mod input;
pub mod render;

pub use {app::TuiApp, input::InputHandler, render::Renderer};

/// TUI mode CLI arguments.
///
/// These arguments configure how the TUI connects to a server.
#[derive(Args, Debug, Clone)]
pub struct TuiArgs {
    /// Connect to server via TCP (e.g., 127.0.0.1:12521).
    #[arg(long, value_name = "ADDR")]
    pub tcp: Option<String>,

    /// Connect to server via Unix socket.
    #[cfg(unix)]
    #[arg(long, value_name = "PATH")]
    pub socket: Option<PathBuf>,
}

impl TuiArgs {
    /// Convert arguments to `ConnectionConfig`.
    #[must_use]
    pub fn into_config(self) -> ConnectionConfig {
        #[cfg(unix)]
        if let Some(path) = self.socket {
            return ConnectionConfig::unix_socket(path);
        }

        self.tcp
            .map_or_else(ConnectionConfig::auto_discover, |addr| {
                ConnectionConfig::tcp_from_addr(&addr)
            })
    }
}
