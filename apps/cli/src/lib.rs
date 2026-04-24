#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! Standalone reovim-cli library.
//!
//! Thin launcher that forwards execution to
//! [`reovim_client_cli::CliArgs::execute`]. CLM v7 locks
//! `clients/cli::CliArgs` as the CLI schema and gRPC execution path;
//! an in-process launcher calls the same entry from code, so all CLI
//! runtime logic lives in the client crate.

use std::io;

pub use reovim_client_cli::CliArgs;

/// Run the standalone CLI flow.
///
/// Delegates to [`reovim_client_cli::CliArgs::execute`], prints the
/// formatted output on success, and maps gRPC client errors onto
/// [`io::Error`]. CLI commands are one-shot requests; no tracing
/// subscriber is installed here (the client crate emits its own
/// diagnostics via `tracing`, which are silently dropped when no
/// subscriber is set).
///
/// # Errors
///
/// Returns an I/O error if the gRPC handshake fails or the requested
/// command fails on the server side.
#[cfg_attr(coverage_nightly, coverage(off))]
pub async fn run(args: CliArgs) -> io::Result<()> {
    match args.execute().await {
        Ok(output) => {
            println!("{output}");
            Ok(())
        }
        Err(e) => Err(io::Error::other(e.to_string())),
    }
}
