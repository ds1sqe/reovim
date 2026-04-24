//! TUI platform `run(args)` entry point.
//!
//! Dispatches to headless or interactive TUI based on `TuiArgs`.
//! Integrated-mode composition (`run_integrated`) is NOT here; an
//! embedded-launcher crate will own that path once it exists.
//!
//! CLM v7 locks `ext/client/platforms/<p>::run(args)` as the bin's
//! dispatch target — the standalone `reovim-tui` bin and any future
//! in-process launcher both drive the TUI through [`run`].

use std::{collections::HashSet, io, path::PathBuf};

use clap::Args;

use crate::{TuiAppError, connect_headless, connect_interactive};

/// CLI arguments for the standalone TUI platform entry point.
///
/// `apps/tui/src/main.rs` uses this type directly as its clap-parsed
/// argument; an in-process launcher constructs it in code to share one
/// schema with the standalone bin.
#[derive(Args, Debug, Clone)]
pub struct TuiArgs {
    /// gRPC server address (host:port).
    #[arg(long, default_value = "127.0.0.1:12540")]
    pub grpc: String,

    /// Run in headless mode (no TTY, for scripting).
    #[arg(long)]
    pub headless: bool,

    /// Viewport width (headless mode only; interactive mode auto-detects).
    #[arg(long, default_value = "120")]
    pub width: u16,

    /// Viewport height (headless mode only; interactive mode auto-detects).
    #[arg(long, default_value = "40")]
    pub height: u16,

    /// Log file path. When unset, logs go to stderr.
    #[arg(long, value_name = "PATH")]
    pub log: Option<PathBuf>,
}

/// Error type returned by the TUI platform `run(args)` entry point.
///
/// Collapses to an `io::Error` via the `From<TuiRunError> for io::Error`
/// impl so the bin-level `apps/tui/src/lib.rs::run` can propagate with
/// `?` into its `io::Result<()>` signature.
#[derive(Debug, thiserror::Error)]
pub enum TuiRunError {
    /// Connection to the gRPC server failed.
    #[error("failed to connect TUI to {addr}: {source}")]
    Connect {
        /// Target gRPC server address.
        addr: String,
        /// Underlying connection error from the TUI app layer.
        #[source]
        source: TuiAppError,
    },

    /// The TUI event loop itself failed.
    #[error("TUI app error: {0}")]
    App(#[source] TuiAppError),

    /// An I/O error occurred outside the app layer (e.g. signal handler).
    #[error(transparent)]
    Io(#[from] io::Error),
}

impl From<TuiRunError> for io::Error {
    fn from(err: TuiRunError) -> Self {
        match err {
            TuiRunError::Io(e) => e,
            TuiRunError::Connect { source, .. } => {
                Self::new(io::ErrorKind::ConnectionRefused, source.to_string())
            }
            TuiRunError::App(source) => Self::other(source.to_string()),
        }
    }
}

/// Run the TUI platform against an already-running gRPC server.
///
/// Dispatches to headless or interactive mode based on `args.headless`.
/// The caller is responsible for initializing logging (see
/// [`crate::logging::init`]) and supplying a tokio runtime.
///
/// # Errors
///
/// Returns [`TuiRunError::Connect`] if the gRPC handshake fails,
/// [`TuiRunError::App`] if the TUI event loop errors, or
/// [`TuiRunError::Io`] if the ctrl-c handler or terminal I/O fails.
pub async fn run(args: TuiArgs) -> Result<(), TuiRunError> {
    // Standalone TUI has no server-side context, so the disabled-
    // extension set is empty. An in-process launcher would compute
    // this set from its bootstrap state and pass it in through a
    // richer entry.
    let disabled: HashSet<String> = HashSet::new();

    if args.headless {
        run_headless(&args.grpc, args.width, args.height, &disabled).await
    } else {
        run_interactive(&args.grpc, &disabled).await
    }
}

/// Run the TUI in headless mode (no TTY; for scripting and tests).
///
/// # Errors
///
/// See [`run`] for the error taxonomy.
#[cfg_attr(coverage_nightly, coverage(off))]
async fn run_headless(
    addr: &str,
    width: u16,
    height: u16,
    disabled: &HashSet<String>,
) -> Result<(), TuiRunError> {
    tracing::info!("Connecting headless TUI to {addr} ({width}x{height})");

    let (mut app, handle) = connect_headless(addr, width, height, None, None, disabled)
        .await
        .map_err(|source| TuiRunError::Connect {
            addr: addr.to_string(),
            source,
        })?;

    tracing::info!("Headless TUI connected and running");

    let app_handle = tokio::spawn(async move { app.run().await });

    tokio::signal::ctrl_c().await?;
    handle.stop().await;

    // Ignore JoinError — event loop already torn down by handle.stop().
    let _ = app_handle.await;

    Ok(())
}

/// Run the TUI in interactive mode attached to the current terminal.
///
/// # Errors
///
/// See [`run`] for the error taxonomy.
#[cfg_attr(coverage_nightly, coverage(off))]
async fn run_interactive(
    addr: &str,
    disabled: &HashSet<String>,
) -> Result<(), TuiRunError> {
    tracing::info!("Connecting interactive TUI to {addr}");

    let (mut app, _handle) = connect_interactive(addr, None, None, disabled)
        .await
        .map_err(|source| TuiRunError::Connect {
            addr: addr.to_string(),
            source,
        })?;

    let result = app.run().await.map_err(TuiRunError::App);
    drop(app);
    result
}
