//! In-process gRPC transport.
//!
//! Serves the reovim tonic stack over a single
//! [`tokio::io::DuplexStream`] connection supplied by an embedded
//! launcher. Tonic 0.12 implements its own `Connected` impl for
//! [`tokio::io::DuplexStream`], so no wrapper is required — the loop
//! here is simply a single-item `Stream<Item = Result<DuplexStream,
//! io::Error>>` handed to the pre-built router's
//! `serve_with_incoming_shutdown`.
//!
//! Shutdown ordering is owned by the caller (the launcher's
//! `ShutdownCoord`): when the shutdown future completes, tonic drains
//! in-flight RPCs and `run` returns.

use std::future::Future;

use {tokio::io::DuplexStream, tonic::transport::server::Router};

/// Serve the pre-built `router` over `stream` until `shutdown` completes.
///
/// # Errors
///
/// Returns the tonic transport error when the service loop stops with
/// a failure (the happy path ends when the shutdown future completes
/// or the stream is dropped).
#[cfg_attr(coverage_nightly, coverage(off))]
pub async fn run<F>(stream: DuplexStream, router: Router, shutdown: F) -> std::io::Result<()>
where
    F: Future<Output = ()> + Send + 'static,
{
    let incoming = tokio_stream::once(Ok::<DuplexStream, std::io::Error>(stream));
    router
        .serve_with_incoming_shutdown(incoming, shutdown)
        .await
        .map_err(std::io::Error::other)
}
