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
mod tests {
    use std::time::Duration;

    use reovim_kernel::api::v1::oneshot;

    use super::*;

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn recv_response_success() {
        let (tx, rx) = oneshot();
        tx.send(42).unwrap();
        let result = recv_response(&rx, Duration::from_secs(1));
        assert_eq!(result.unwrap(), 42);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn recv_response_timeout() {
        let (_tx, rx) = oneshot::<i32>();
        let result = recv_response(&rx, Duration::from_millis(10));
        assert!(result.is_err());
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn recv_response_sender_dropped() {
        let (tx, rx) = oneshot::<i32>();
        drop(tx);
        let result = recv_response(&rx, Duration::from_secs(1));
        assert!(result.is_err());
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn recv_response_with_complex_type() {
        let (tx, rx) = oneshot::<Result<Option<String>, String>>();
        tx.send(Ok(Some("hello".to_string()))).unwrap();
        let result = recv_response(&rx, Duration::from_secs(1));
        assert_eq!(result.unwrap().unwrap().unwrap(), "hello");
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn recv_response_concurrent_send() {
        let (tx, rx) = oneshot();
        tokio::task::spawn_blocking(move || {
            std::thread::sleep(Duration::from_millis(50));
            tx.send(99).unwrap();
        });
        let result = recv_response(&rx, Duration::from_secs(2));
        assert_eq!(result.unwrap(), 99);
    }
}
