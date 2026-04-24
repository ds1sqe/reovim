#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! Standalone reovim-web library.
//!
//! Ships the bin + lib shape; the SSR runtime is not yet implemented
//! and `run` returns a structured `Unsupported` error so callers can
//! distinguish "intentionally unimplemented" from "crashed".

use std::{io, net::SocketAddr};

use clap::Args;

/// CLI arguments for the standalone `reovim-web` bin.
///
/// Minimal today — just a listen address. Grows with TLS, SSR-routing,
/// and static-asset-path flags when the SSR runtime lands.
#[derive(Args, Debug, Clone)]
pub struct WebArgs {
    /// Socket address to bind the HTTP server on.
    #[arg(long, value_name = "ADDR", default_value = "127.0.0.1:7870")]
    pub listen: SocketAddr,
}

/// Run the standalone web flow.
///
/// Currently returns `Unsupported`; the SSR runtime will be supplied
/// once the strategy is finalized.
///
/// # Errors
///
/// Always returns [`io::ErrorKind::Unsupported`] — the runtime is
/// not yet implemented.
// TODO(#769): SSR strategy.
pub fn run(_args: WebArgs) -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "reovim-web: SSR runtime not yet implemented (#769)",
    ))
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
