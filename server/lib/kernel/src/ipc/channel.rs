//! Channel abstractions for inter-subsystem communication.
//!
//! This module provides thin wrappers around `std::sync::mpsc` channels,
//! keeping the kernel runtime-agnostic (no tokio dependency).
//!
//! # Design Philosophy
//!
//! Following the "mechanisms not policy" principle:
//! - Provides basic channel primitives
//! - No opinion on how they should be used
//! - Runtime integration is left to higher layers
//!
//! # Channel Types
//!
//! - **Unbounded MPSC**: Multiple producers, single consumer, unlimited buffer
//! - **Bounded MPSC**: Multiple producers, single consumer, fixed capacity
//! - **Oneshot**: Single-use channel for request-response patterns
//!
//! # Example
//!
//! ```
//! use reovim_kernel::api::v1::*;
//!
//! // Unbounded channel
//! let (tx, rx) = channel::<i32>();
//! tx.send(42).unwrap();
//! assert_eq!(rx.recv().unwrap(), 42);
//!
//! // Bounded channel with capacity 2
//! let (tx, rx) = bounded::<i32>(2);
//! tx.send(1).unwrap();
//! tx.send(2).unwrap();
//! // tx.send(3) would block until space is available
//!
//! // Oneshot for request-response
//! let (tx, rx) = oneshot::<String>();
//! tx.send("response".to_string()).unwrap();
//! assert_eq!(rx.recv().unwrap(), "response");
//! ```

use std::{sync::mpsc, time::Duration};

use reovim_arch::sync::Mutex;

// ============================================================================
// Unbounded Channel
// ============================================================================

/// Sender for unbounded MPSC channel.
///
/// Cloning creates a new sender to the same channel.
pub struct Sender<T>(mpsc::Sender<T>);

// Manual Clone impl because mpsc::Sender<T> is Clone without requiring T: Clone
impl<T> Clone for Sender<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

/// Receiver for unbounded MPSC channel.
///
/// Only one receiver exists per channel.
pub struct Receiver<T>(mpsc::Receiver<T>);

/// Error returned when sending fails because the receiver was dropped.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SendError<T>(pub T);

impl<T> std::fmt::Display for SendError<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "sending on a closed channel")
    }
}

impl<T: std::fmt::Debug> std::error::Error for SendError<T> {}

/// Error returned when receiving fails because the channel is empty and closed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RecvError;

impl std::fmt::Display for RecvError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "receiving on a closed channel")
    }
}

impl std::error::Error for RecvError {}

/// Error returned when `try_recv` fails.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TryRecvError {
    /// No message available.
    Empty,
    /// Channel is closed.
    Disconnected,
}

impl std::fmt::Display for TryRecvError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Empty => write!(f, "channel is empty"),
            Self::Disconnected => write!(f, "channel is disconnected"),
        }
    }
}

impl std::error::Error for TryRecvError {}

impl<T> Sender<T> {
    /// Send a value through the channel.
    ///
    /// # Errors
    ///
    /// Returns `Err(SendError(value))` if the receiver has been dropped.
    pub fn send(&self, value: T) -> Result<(), SendError<T>> {
        self.0.send(value).map_err(|e| SendError(e.0))
    }
}

impl<T> std::fmt::Debug for Sender<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Sender").finish_non_exhaustive()
    }
}

impl<T> Receiver<T> {
    /// Block until a value is received.
    ///
    /// # Errors
    ///
    /// Returns `Err(RecvError)` if the channel is closed.
    pub fn recv(&self) -> Result<T, RecvError> {
        self.0.recv().map_err(|_| RecvError)
    }

    /// Try to receive without blocking.
    ///
    /// # Errors
    ///
    /// Returns `Err(TryRecvError::Empty)` if no message is available,
    /// or `Err(TryRecvError::Disconnected)` if the channel is closed.
    pub fn try_recv(&self) -> Result<T, TryRecvError> {
        self.0.try_recv().map_err(|e| match e {
            mpsc::TryRecvError::Empty => TryRecvError::Empty,
            mpsc::TryRecvError::Disconnected => TryRecvError::Disconnected,
        })
    }

    /// Block until a value is received or timeout expires.
    ///
    /// # Errors
    ///
    /// Returns `Err(RecvError)` on timeout or if channel is closed.
    pub fn recv_timeout(&self, timeout: Duration) -> Result<T, RecvError> {
        self.0.recv_timeout(timeout).map_err(|_| RecvError)
    }

    /// Create an iterator over received values.
    pub fn iter(&self) -> impl Iterator<Item = T> + '_ {
        self.0.iter()
    }

    /// Try to receive all available values without blocking.
    pub fn try_iter(&self) -> impl Iterator<Item = T> + '_ {
        self.0.try_iter()
    }
}

impl<T> std::fmt::Debug for Receiver<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Receiver").finish_non_exhaustive()
    }
}

/// Create an unbounded MPSC channel.
///
/// # Example
///
/// ```
/// use reovim_kernel::api::v1::*;
///
/// let (tx, rx) = channel::<i32>();
/// tx.send(42).unwrap();
/// assert_eq!(rx.recv().unwrap(), 42);
/// ```
#[must_use]
pub fn channel<T>() -> (Sender<T>, Receiver<T>) {
    let (tx, rx) = mpsc::channel();
    (Sender(tx), Receiver(rx))
}

// ============================================================================
// Bounded Channel
// ============================================================================

/// Sender for bounded MPSC channel.
pub struct BoundedSender<T>(mpsc::SyncSender<T>);

// Manual Clone impl because mpsc::SyncSender<T> is Clone without requiring T: Clone
impl<T> Clone for BoundedSender<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

/// Receiver for bounded MPSC channel (same as unbounded).
pub struct BoundedReceiver<T>(mpsc::Receiver<T>);

/// Error returned when `try_send` fails.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrySendError<T> {
    /// Channel is full.
    Full(T),
    /// Channel is closed.
    Disconnected(T),
}

impl<T> std::fmt::Display for TrySendError<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Full(_) => write!(f, "channel is full"),
            Self::Disconnected(_) => write!(f, "channel is disconnected"),
        }
    }
}

impl<T: std::fmt::Debug> std::error::Error for TrySendError<T> {}

impl<T> BoundedSender<T> {
    /// Send a value, blocking if the channel is full.
    ///
    /// # Errors
    ///
    /// Returns `Err(SendError(value))` if the receiver has been dropped.
    pub fn send(&self, value: T) -> Result<(), SendError<T>> {
        self.0.send(value).map_err(|e| SendError(e.0))
    }

    /// Try to send without blocking.
    ///
    /// # Errors
    ///
    /// Returns `Err(TrySendError::Full(value))` if the channel is full,
    /// or `Err(TrySendError::Disconnected(value))` if the receiver was dropped.
    pub fn try_send(&self, value: T) -> Result<(), TrySendError<T>> {
        self.0.try_send(value).map_err(|e| match e {
            mpsc::TrySendError::Full(v) => TrySendError::Full(v),
            mpsc::TrySendError::Disconnected(v) => TrySendError::Disconnected(v),
        })
    }
}

impl<T> std::fmt::Debug for BoundedSender<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BoundedSender").finish_non_exhaustive()
    }
}

impl<T> BoundedReceiver<T> {
    /// Block until a value is received.
    ///
    /// # Errors
    ///
    /// Returns `Err(RecvError)` if the channel is closed.
    pub fn recv(&self) -> Result<T, RecvError> {
        self.0.recv().map_err(|_| RecvError)
    }

    /// Try to receive without blocking.
    ///
    /// # Errors
    ///
    /// Returns `Err(TryRecvError::Empty)` if no message is available,
    /// or `Err(TryRecvError::Disconnected)` if the channel is closed.
    pub fn try_recv(&self) -> Result<T, TryRecvError> {
        self.0.try_recv().map_err(|e| match e {
            mpsc::TryRecvError::Empty => TryRecvError::Empty,
            mpsc::TryRecvError::Disconnected => TryRecvError::Disconnected,
        })
    }

    /// Block until a value is received or timeout expires.
    ///
    /// # Errors
    ///
    /// Returns `Err(RecvError)` on timeout or if the channel is closed.
    pub fn recv_timeout(&self, timeout: Duration) -> Result<T, RecvError> {
        self.0.recv_timeout(timeout).map_err(|_| RecvError)
    }

    /// Create an iterator over received values.
    pub fn iter(&self) -> impl Iterator<Item = T> + '_ {
        self.0.iter()
    }

    /// Try to receive all available values without blocking.
    pub fn try_iter(&self) -> impl Iterator<Item = T> + '_ {
        self.0.try_iter()
    }
}

impl<T> std::fmt::Debug for BoundedReceiver<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BoundedReceiver").finish_non_exhaustive()
    }
}

/// Create a bounded MPSC channel with the specified capacity.
///
/// Senders will block when the channel is full.
///
/// # Example
///
/// ```
/// use reovim_kernel::api::v1::*;
///
/// let (tx, rx) = bounded::<i32>(2);
/// tx.send(1).unwrap();
/// tx.send(2).unwrap();
/// // tx.send(3) would block until space is available
///
/// assert_eq!(rx.recv().unwrap(), 1);
/// assert_eq!(rx.recv().unwrap(), 2);
/// ```
#[must_use]
pub fn bounded<T>(capacity: usize) -> (BoundedSender<T>, BoundedReceiver<T>) {
    let (tx, rx) = mpsc::sync_channel(capacity);
    (BoundedSender(tx), BoundedReceiver(rx))
}

// ============================================================================
// Oneshot Channel
// ============================================================================

/// Sender for oneshot channel.
///
/// Can only send a single value.
pub struct OneshotSender<T> {
    inner: Mutex<Option<mpsc::SyncSender<T>>>,
}

/// Receiver for oneshot channel.
///
/// Can only receive a single value.
pub struct OneshotReceiver<T>(mpsc::Receiver<T>);

impl<T> OneshotSender<T> {
    /// Send the value, consuming the sender.
    ///
    /// This can only be called once.
    ///
    /// # Errors
    ///
    /// Returns `Err(value)` if the receiver has been dropped.
    pub fn send(self, value: T) -> Result<(), T> {
        let sender = self.inner.lock().take();
        match sender {
            Some(tx) => tx.send(value).map_err(|e| e.0),
            None => Err(value),
        }
    }

    /// Check if the receiver is still waiting.
    #[must_use]
    pub fn is_connected(&self) -> bool {
        self.inner.lock().is_some()
    }
}

impl<T> std::fmt::Debug for OneshotSender<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OneshotSender")
            .field("connected", &self.is_connected())
            .finish()
    }
}

impl<T> OneshotReceiver<T> {
    /// Block until the value is received.
    ///
    /// # Errors
    ///
    /// Returns `Err(RecvError)` if the sender was dropped without sending.
    pub fn recv(self) -> Result<T, RecvError> {
        self.0.recv().map_err(|_| RecvError)
    }

    /// Try to receive without blocking.
    ///
    /// # Errors
    ///
    /// Returns `Err(TryRecvError::Empty)` if no value is available yet,
    /// or `Err(TryRecvError::Disconnected)` if the sender was dropped.
    pub fn try_recv(&self) -> Result<T, TryRecvError> {
        self.0.try_recv().map_err(|e| match e {
            mpsc::TryRecvError::Empty => TryRecvError::Empty,
            mpsc::TryRecvError::Disconnected => TryRecvError::Disconnected,
        })
    }

    /// Block until received or timeout expires.
    ///
    /// # Errors
    ///
    /// Returns `Err(RecvError)` on timeout or if the sender was dropped.
    pub fn recv_timeout(&self, timeout: Duration) -> Result<T, RecvError> {
        self.0.recv_timeout(timeout).map_err(|_| RecvError)
    }
}

impl<T> std::fmt::Debug for OneshotReceiver<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OneshotReceiver").finish_non_exhaustive()
    }
}

/// Create a oneshot channel for single-value communication.
///
/// Useful for request-response patterns where exactly one response is expected.
///
/// # Example
///
/// ```
/// use reovim_kernel::api::v1::*;
/// use std::thread;
///
/// let (tx, rx) = oneshot::<String>();
///
/// thread::spawn(move || {
///     tx.send("response".to_string()).unwrap();
/// });
///
/// assert_eq!(rx.recv().unwrap(), "response");
/// ```
#[must_use]
pub fn oneshot<T>() -> (OneshotSender<T>, OneshotReceiver<T>) {
    let (tx, rx) = mpsc::sync_channel(1);
    (
        OneshotSender {
            inner: Mutex::new(Some(tx)),
        },
        OneshotReceiver(rx),
    )
}

#[cfg(test)]
mod tests {
    use {super::*, std::thread};

    // ========== Unbounded channel tests ==========

    #[test]
    fn test_channel_send_recv() {
        let (tx, rx) = channel::<i32>();
        tx.send(42).unwrap();
        assert_eq!(rx.recv().unwrap(), 42);
    }

    #[test]
    fn test_channel_multiple_sends() {
        let (tx, rx) = channel::<i32>();
        tx.send(1).unwrap();
        tx.send(2).unwrap();
        tx.send(3).unwrap();
        assert_eq!(rx.recv().unwrap(), 1);
        assert_eq!(rx.recv().unwrap(), 2);
        assert_eq!(rx.recv().unwrap(), 3);
    }

    #[test]
    fn test_channel_clone_sender() {
        let (tx, rx) = channel::<i32>();
        let tx2 = tx.clone();
        tx.send(1).unwrap();
        tx2.send(2).unwrap();
        let mut values = vec![rx.recv().unwrap(), rx.recv().unwrap()];
        values.sort_unstable();
        assert_eq!(values, vec![1, 2]);
    }

    #[test]
    fn test_channel_try_recv_empty() {
        let (tx, rx) = channel::<i32>();
        assert_eq!(rx.try_recv(), Err(TryRecvError::Empty));
        tx.send(42).unwrap();
        assert_eq!(rx.try_recv(), Ok(42));
    }

    #[test]
    fn test_channel_recv_disconnected() {
        let (tx, rx) = channel::<i32>();
        drop(tx);
        assert_eq!(rx.recv(), Err(RecvError));
    }

    #[test]
    fn test_channel_send_disconnected() {
        let (tx, rx) = channel::<i32>();
        drop(rx);
        assert_eq!(tx.send(42), Err(SendError(42)));
    }

    #[test]
    fn test_channel_recv_timeout() {
        let (tx, rx) = channel::<i32>();
        let start = std::time::Instant::now();
        let result = rx.recv_timeout(Duration::from_millis(10));
        assert!(result.is_err());
        assert!(start.elapsed() >= Duration::from_millis(10));

        tx.send(42).unwrap();
        assert_eq!(rx.recv_timeout(Duration::from_secs(1)).unwrap(), 42);
    }

    #[test]
    fn test_channel_try_iter() {
        let (tx, rx) = channel::<i32>();
        tx.send(1).unwrap();
        tx.send(2).unwrap();
        tx.send(3).unwrap();
        let values: Vec<_> = rx.try_iter().collect();
        assert_eq!(values, vec![1, 2, 3]);
    }

    // ========== Bounded channel tests ==========

    #[test]
    fn test_bounded_send_recv() {
        let (tx, rx) = bounded::<i32>(2);
        tx.send(1).unwrap();
        tx.send(2).unwrap();
        assert_eq!(rx.recv().unwrap(), 1);
        assert_eq!(rx.recv().unwrap(), 2);
    }

    #[test]
    fn test_bounded_try_send_full() {
        let (tx, rx) = bounded::<i32>(1);
        tx.send(1).unwrap();
        assert!(matches!(tx.try_send(2), Err(TrySendError::Full(2))));

        rx.recv().unwrap();
        tx.try_send(2).unwrap();
    }

    #[test]
    fn test_bounded_try_send_disconnected() {
        let (tx, rx) = bounded::<i32>(1);
        drop(rx);
        assert!(matches!(tx.try_send(42), Err(TrySendError::Disconnected(42))));
    }

    #[test]
    fn test_bounded_clone_sender() {
        let (tx, rx) = bounded::<i32>(2);
        let tx2 = tx.clone();
        tx.send(1).unwrap();
        tx2.send(2).unwrap();
        assert_eq!(rx.recv().unwrap(), 1);
        assert_eq!(rx.recv().unwrap(), 2);
    }

    // ========== Oneshot channel tests ==========

    #[test]
    fn test_oneshot_send_recv() {
        let (tx, rx) = oneshot::<String>();
        tx.send("hello".to_string()).unwrap();
        assert_eq!(rx.recv().unwrap(), "hello");
    }

    #[test]
    fn test_oneshot_receiver_dropped() {
        let (tx, rx) = oneshot::<i32>();
        drop(rx);
        assert_eq!(tx.send(42), Err(42));
    }

    #[test]
    fn test_oneshot_sender_dropped() {
        let (tx, rx) = oneshot::<i32>();
        drop(tx);
        assert_eq!(rx.recv(), Err(RecvError));
    }

    #[test]
    fn test_oneshot_try_recv() {
        let (tx, rx) = oneshot::<i32>();
        assert_eq!(rx.try_recv(), Err(TryRecvError::Empty));
        tx.send(42).unwrap();
        assert_eq!(rx.try_recv(), Ok(42));
    }

    #[test]
    fn test_oneshot_is_connected() {
        let (tx, rx) = oneshot::<i32>();
        assert!(tx.is_connected());
        drop(rx);
        // Note: is_connected doesn't detect receiver drop before send
        // (limitation of mpsc::SyncSender)
        let _ = tx; // Silence unused variable warning
    }

    #[test]
    fn test_oneshot_thread() {
        let (tx, rx) = oneshot::<String>();

        let handle = thread::spawn(move || {
            thread::sleep(Duration::from_millis(10));
            tx.send("done".to_string()).unwrap();
        });

        assert_eq!(rx.recv().unwrap(), "done");
        handle.join().unwrap();
    }

    // ========== Error display tests ==========

    #[test]
    fn test_error_display() {
        assert!(SendError(42).to_string().contains("closed"));
        assert!(RecvError.to_string().contains("closed"));
        assert!(TryRecvError::Empty.to_string().contains("empty"));
        assert!(
            TryRecvError::Disconnected
                .to_string()
                .contains("disconnected")
        );
        assert!(TrySendError::Full(42).to_string().contains("full"));
        assert!(
            TrySendError::Disconnected(42)
                .to_string()
                .contains("disconnected")
        );
    }

    // ========== Debug tests ==========

    #[test]
    fn test_debug_impls() {
        let (tx, rx) = channel::<i32>();
        let _ = format!("{tx:?}");
        let _ = format!("{rx:?}");

        let (tx, rx) = bounded::<i32>(1);
        let _ = format!("{tx:?}");
        let _ = format!("{rx:?}");

        let (tx, rx) = oneshot::<i32>();
        let _ = format!("{tx:?}");
        let _ = format!("{rx:?}");
    }

    // === BoundedReceiver recv_timeout ===

    #[test]
    fn test_bounded_recv_timeout() {
        let (tx, rx) = bounded::<i32>(2);
        let start = std::time::Instant::now();
        let result = rx.recv_timeout(Duration::from_millis(10));
        assert!(result.is_err());
        assert!(start.elapsed() >= Duration::from_millis(10));

        tx.send(42).unwrap();
        assert_eq!(rx.recv_timeout(Duration::from_secs(1)).unwrap(), 42);
    }

    // === BoundedReceiver iter ===

    #[test]
    fn test_bounded_iter() {
        let (tx, rx) = bounded::<i32>(5);
        tx.send(1).unwrap();
        tx.send(2).unwrap();
        tx.send(3).unwrap();
        drop(tx); // close the channel so iter terminates

        let values: Vec<_> = rx.iter().collect();
        assert_eq!(values, vec![1, 2, 3]);
    }

    // === BoundedReceiver try_iter ===

    #[test]
    fn test_bounded_try_iter() {
        let (tx, rx) = bounded::<i32>(5);
        tx.send(10).unwrap();
        tx.send(20).unwrap();
        let values: Vec<_> = rx.try_iter().collect();
        assert_eq!(values, vec![10, 20]);
    }

    // === BoundedSender send disconnected ===

    #[test]
    fn test_bounded_send_disconnected() {
        let (tx, rx) = bounded::<i32>(2);
        drop(rx);
        assert!(tx.send(42).is_err());
    }

    // === BoundedReceiver try_recv disconnected ===

    #[test]
    fn test_bounded_try_recv_disconnected() {
        let (tx, rx) = bounded::<i32>(2);
        drop(tx);
        assert_eq!(rx.try_recv(), Err(TryRecvError::Disconnected));
    }

    // === Receiver iter ===

    #[test]
    fn test_receiver_iter() {
        let (tx, rx) = channel::<i32>();
        tx.send(1).unwrap();
        tx.send(2).unwrap();
        drop(tx); // close channel

        let values: Vec<_> = rx.iter().collect();
        assert_eq!(values, vec![1, 2]);
    }

    // === OneshotReceiver recv_timeout ===

    #[test]
    fn test_oneshot_recv_timeout() {
        let (_tx, rx) = oneshot::<i32>();
        let start = std::time::Instant::now();
        let result = rx.recv_timeout(Duration::from_millis(10));
        assert!(result.is_err());
        assert!(start.elapsed() >= Duration::from_millis(10));

        // Now send and verify recv_timeout works
        let (tx2, rx2) = oneshot::<i32>();
        tx2.send(42).unwrap();
        assert_eq!(rx2.recv_timeout(Duration::from_secs(1)).unwrap(), 42);
    }

    // === OneshotReceiver try_recv disconnected ===

    #[test]
    fn test_oneshot_try_recv_disconnected() {
        let (tx, rx) = oneshot::<i32>();
        drop(tx);
        assert_eq!(rx.try_recv(), Err(TryRecvError::Disconnected));
    }

    // === Error trait impls ===

    #[test]
    fn test_send_error_is_std_error() {
        let err: Box<dyn std::error::Error> = Box::new(SendError(42));
        assert!(err.to_string().contains("closed"));
    }

    #[test]
    fn test_recv_error_is_std_error() {
        let err: Box<dyn std::error::Error> = Box::new(RecvError);
        assert!(err.to_string().contains("closed"));
    }

    #[test]
    fn test_try_recv_error_is_std_error() {
        let err: Box<dyn std::error::Error> = Box::new(TryRecvError::Empty);
        assert!(err.to_string().contains("empty"));
    }

    #[test]
    fn test_try_send_error_is_std_error() {
        let err: Box<dyn std::error::Error> = Box::new(TrySendError::Full(42));
        assert!(err.to_string().contains("full"));
    }
}
