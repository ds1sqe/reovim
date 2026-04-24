//! OS-pipe gRPC transport.
//!
//! Will eventually run the tonic gRPC stack over a pair of
//! `AsyncRead` / `AsyncWrite` handles (typically the subprocess
//! child's stdio pair, or the standalone bin's `stdin` / `stdout`).
//! The service assembly is deferred (tracked under #769); the
//! current body is a placeholder that surfaces the deferral as an
//! `io::Error::other`.

use tokio::io::{AsyncRead, AsyncWrite};

/// Drives the server's gRPC stack over a read/write handle pair.
///
/// # Errors
///
/// Placeholder — always returns `io::Error::other(..)` until the
/// tonic service assembly is wired. Tracked under #769.
// Async signature reserved for the future tonic service assembly — a
// sync stub would force a breaking signature change later.
#[allow(clippy::unused_async)]
pub async fn run<R, W>(_read: R, _write: W) -> std::io::Result<()>
where
    R: AsyncRead + Send + Unpin + 'static,
    W: AsyncWrite + Send + Unpin + 'static,
{
    Err(std::io::Error::other("pipe transport not yet wired"))
}
