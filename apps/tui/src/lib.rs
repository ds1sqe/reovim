#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! Standalone reovim-tui library.
//!
//! Thin launcher that initializes file logging via the platform crate
//! and forwards execution to `reovim_client_ext_platform_tui::run`.
//! CLM v7 locks `ext/client/platforms/<p>::run(args)` as the bin's
//! dispatch target; an in-process launcher calls the same platform
//! entry from code, so all TUI runtime logic (including logging init)
//! lives in the platform crate.

use std::io;

pub use reovim_client_ext_platform_tui::TuiArgs;

/// Run the standalone TUI flow.
///
/// Initializes file logging using `args.log` (stderr if unset) via
/// [`reovim_client_ext_platform_tui::logging::init`], then delegates
/// to [`reovim_client_ext_platform_tui::run`].
///
/// # Errors
///
/// Returns an I/O error if logging initialization fails, the gRPC
/// handshake fails, or the TUI event loop errors.
#[cfg_attr(coverage_nightly, coverage(off))]
pub async fn run(args: TuiArgs) -> io::Result<()> {
    reovim_client_ext_platform_tui::logging::init(args.log.as_deref())?;
    reovim_client_ext_platform_tui::run(args)
        .await
        .map_err(io::Error::from)
}

/// Run the standalone TUI flow over a pre-built in-process
/// [`tokio::io::DuplexStream`].
///
/// Used by the embedded launcher to speak tonic-over-duplex to an
/// in-process `reovim-server`. Logging is NOT initialized here — the
/// launcher owns the terminal and logging target — so the caller must
/// set up their own subscriber if they want TUI-side tracing.
///
/// # Errors
///
/// Returns an I/O error if the gRPC handshake fails or the TUI event
/// loop errors.
#[cfg_attr(coverage_nightly, coverage(off))]
pub async fn run_with_stream(stream: tokio::io::DuplexStream, args: TuiArgs) -> io::Result<()> {
    reovim_client_ext_platform_tui::run_with_stream(stream, args)
        .await
        .map_err(io::Error::from)
}
