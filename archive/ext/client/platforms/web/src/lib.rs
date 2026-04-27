#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! Reovim Web platform runtime (Flight 76 spike).
//!
//! Wave 3a (#771) note: this platform is intentionally NOT wired into
//! the package-manager runtime loader. The Web target is a WASM
//! cdylib root with no host filesystem — `pkg-runtime-loader` reads
//! `pkg.lock` from a filesystem path, and `dylib-loader::Library::open`
//! does not apply to WASM modules. Wave 3a wires only OS-dylib
//! platforms; a WASM-aware loader is out of scope until post-Wave-3c.
//!
//! Serves the canonical frame as an inline SVG inside a minimal HTML5
//! document over HTTP/1.1. Exposes three orthogonal pieces:
//!
//! - [`frame`] — self-contained cell-grid primitives ([`WebFrame`],
//!   [`WebCell`], [`WebColor`]) and the [`canonical_frame`] fixture.
//! - [`svg`] — pure-function SVG / HTML rendering
//!   ([`render_frame_svg`], [`render_page`]).
//! - [`server`] — hand-rolled HTTP/1.1 responder ([`serve`]) built on
//!   `tokio::net::TcpListener`. 8 KiB header read cap.
//!
//! The [`run`] entry point ties all three together: binds `args.listen`,
//! serves `render_page(&canonical_frame())` on every request, and
//! shuts down on `ctrl_c`.
//!
//! ## CLM v7 boundary
//!
//! This crate is an `ext/client/platforms/*` — the locked layer model
//! forbids it from depending on `ext/client/capabilities/*` or
//! `ext/client/driver/*`. The cell-grid primitives in [`frame`] are
//! therefore self-contained rather than reusing
//! `reovim_ext_client_tui_cap_cell::CellCapability`. The duplication
//! is ≤60 LOC and is deferred to Phase E.1 of #753 for unification.

use std::net::SocketAddr;

use clap::Args;

pub mod frame;
pub mod server;
pub mod svg;

pub use {
    frame::{WebCell, WebColor, WebFrame, canonical_frame},
    server::{serve, serve_with_listener},
    svg::{render_frame_svg, render_page},
};

#[cfg(test)]
#[path = "frame_tests.rs"]
mod frame_tests;
#[cfg(test)]
#[path = "server_tests.rs"]
mod server_tests;
#[cfg(test)]
#[path = "svg_tests.rs"]
mod svg_tests;

/// CLI arguments for the standalone `reovim-web` bin.
#[derive(Args, Debug, Clone)]
pub struct WebArgs {
    /// Socket address to bind the HTTP server on.
    #[arg(long, value_name = "ADDR", default_value = "127.0.0.1:7870")]
    pub listen: SocketAddr,
}

/// Error surfaced by [`run`].
#[derive(Debug, thiserror::Error)]
pub enum RunError {
    /// Failure binding the listener or serving a connection.
    #[error("web server I/O error: {0}")]
    Io(#[from] std::io::Error),
}

/// Run the Web platform: bind `args.listen`, serve the canonical frame
/// on every request, shut down on `ctrl_c`.
///
/// # Errors
///
/// Returns [`RunError::Io`] if the listener cannot be bound.
///
/// # Panics
///
/// Panics if a tokio current-thread runtime cannot be constructed; the
/// expected failure mode is OS resource exhaustion, not a programming
/// error.
#[cfg_attr(coverage_nightly, coverage(off))]
#[allow(clippy::needless_pass_by_value)] // mirrors the platform `run(args)` convention
pub fn run(args: WebArgs) -> Result<(), RunError> {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("tokio current-thread runtime");
    rt.block_on(serve(args.listen, || render_page(&canonical_frame()), async {
        let _ = tokio::signal::ctrl_c().await;
    }))?;
    Ok(())
}
