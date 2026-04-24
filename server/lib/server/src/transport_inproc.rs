//! In-process gRPC transport.
//!
//! Will eventually run the tonic gRPC stack over a single
//! `DuplexStream` connection handed in by an embedded launcher. The
//! service assembly is deferred (tracked under #769); the current
//! body is a placeholder that surfaces the deferral as an
//! `io::Error::other`.

use tokio::io::DuplexStream;

/// Drives the server's gRPC stack over a single `DuplexStream`
/// connection (the server-side end of a
/// [`crate::inproc_channel_pair`] duplex).
///
/// # Errors
///
/// Placeholder — always returns `io::Error::other(..)` until the
/// tonic service assembly is wired. Tracked under #769.
// Async signature reserved for the future tonic service assembly — a
// sync stub would force a breaking signature change later.
#[allow(clippy::unused_async)]
pub async fn run(_stream: DuplexStream) -> std::io::Result<()> {
    Err(std::io::Error::other("inproc transport not yet wired"))
}
