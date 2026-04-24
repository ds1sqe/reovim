//! In-process gRPC transport.
//!
//! Serves the reovim tonic stack over a single
//! [`tokio::io::DuplexStream`] connection supplied by an embedded
//! launcher. Tonic 0.12 implements its own `Connected` impl for
//! [`tokio::io::DuplexStream`], so no wrapper is required — the loop
//! here is simply a single-item `Stream<Item = Result<DuplexStream,
//! io::Error>>` handed to the pre-built router's
//! `serve_with_incoming`.
//!
//! Graceful shutdown (broadcast signal + server drain) is the 2b.E
//! deliverable; this path relies on the launcher aborting the tokio
//! task that owns `run(..)` when the client side has exited, which
//! drops the `DuplexStream` and ends the stream.

use {tokio::io::DuplexStream, tonic::transport::server::Router};

/// Serve the pre-built `router` over `stream`.
///
/// # Errors
///
/// Returns the tonic transport error when the service loop stops with
/// a failure (the happy path ends when the stream is dropped and no
/// new connections are produced).
#[cfg_attr(coverage_nightly, coverage(off))]
pub async fn run(stream: DuplexStream, router: Router) -> std::io::Result<()> {
    let incoming = tokio_stream::once(Ok::<DuplexStream, std::io::Error>(stream));
    router
        .serve_with_incoming(incoming)
        .await
        .map_err(std::io::Error::other)
}
