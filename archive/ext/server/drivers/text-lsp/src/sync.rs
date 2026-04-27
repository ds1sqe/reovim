//! Synchronous helpers for LSP request/response bridging.
//!
//! Command handlers run synchronously on tokio worker threads. Blocking
//! directly with `recv_timeout` can starve the tokio runtime — if the
//! saturator's `tokio::select!` loop runs on a worker thread that never
//! gets scheduled, the request is never processed and the oneshot sender
//! is dropped, producing a spurious "channel closed" error.
//!
//! [`recv_response`] wraps the blocking wait in [`tokio::task::block_in_place`],
//! which tells the tokio scheduler to move other tasks off the current thread
//! while it blocks. This prevents thread starvation and allows the saturator
//! to process the request concurrently.

use std::time::Duration;

use reovim_kernel::api::v1::{OneshotReceiver, RecvError};

/// Receive an LSP response synchronously without starving the tokio runtime.
///
/// Uses [`tokio::task::block_in_place`] to yield the current worker thread
/// to other async tasks while blocking on the oneshot channel. This is
/// essential when calling from a synchronous `CommandHandler::execute()`
/// that runs on a tokio worker thread.
///
/// # Errors
///
/// Returns `Err(RecvError)` if the sender was dropped or the timeout expires.
///
/// # Panics
///
/// Panics if called from within a `current_thread` runtime (same as
/// `block_in_place`). Reovim uses `multi_thread` so this is safe.
pub fn recv_response<T>(rx: &OneshotReceiver<T>, timeout: Duration) -> Result<T, RecvError> {
    tokio::task::block_in_place(|| rx.recv_timeout(timeout))
}

#[cfg(test)]
#[path = "sync_tests.rs"]
mod tests;
