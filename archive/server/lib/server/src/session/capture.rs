//! TUI capture request tracking for CLI → Server → TUI → Server → CLI relay.
//!
//! Manages pending capture requests and correlates responses from TUI clients.
//!
//! # Flow
//!
//! 1. CLI sends `GetScreenContent` RPC request to server
//! 2. Server creates a pending request with a unique ID and `oneshot::Sender`
//! 3. Server sends `capture_request` notification to TUI client
//! 4. TUI client processes request and sends `capture_response` notification
//! 5. Server receives notification, finds pending request, sends result via channel
//! 6. Original handler receives result and responds to CLI

use std::{
    collections::HashMap,
    sync::atomic::{AtomicU64, Ordering},
    time::Duration,
};

use {parking_lot::RwLock, tokio::sync::oneshot};

/// Default timeout for capture requests.
pub const CAPTURE_TIMEOUT_SECS: u64 = 5;

/// Error type for capture operations.
#[derive(Debug)]
pub enum CaptureError {
    /// No TUI client connected to handle the capture.
    NoTuiClient,
    /// Capture request timed out.
    Timeout,
    /// TUI client disconnected during capture.
    Disconnected,
    /// Invalid response from TUI.
    InvalidResponse(String),
}

impl std::fmt::Display for CaptureError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoTuiClient => {
                write!(f, "No TUI client connected. Start TUI first with `reovim tui`")
            }
            Self::Timeout => {
                write!(f, "TUI capture timeout ({CAPTURE_TIMEOUT_SECS}s). TUI may be frozen")
            }
            Self::Disconnected => write!(f, "TUI client disconnected during capture"),
            Self::InvalidResponse(msg) => write!(f, "Invalid capture response: {msg}"),
        }
    }
}

impl std::error::Error for CaptureError {}

/// Captured screen content result.
#[derive(Debug, Clone)]
pub struct CaptureResult {
    /// Frame width.
    pub width: u64,
    /// Frame height.
    pub height: u64,
    /// Format used for capture.
    pub format: String,
    /// Captured frame content.
    pub content: String,
}

/// Tracks pending capture requests and correlates responses.
///
/// Thread-safe structure that allows concurrent request creation and response delivery.
/// Uses atomic counters for unique request IDs and `RwLock` for the pending request map.
pub struct CaptureTracker {
    /// Next request ID.
    next_id: AtomicU64,
    /// Pending requests waiting for responses.
    pending: RwLock<HashMap<u64, PendingCapture>>,
}

/// A pending capture request waiting for a response.
struct PendingCapture {
    /// Channel to deliver the result.
    sender: oneshot::Sender<CaptureResult>,
}

impl CaptureTracker {
    /// Create a new capture tracker.
    #[must_use]
    pub fn new() -> Self {
        Self {
            next_id: AtomicU64::new(1),
            pending: RwLock::new(HashMap::new()),
        }
    }

    /// Create a pending capture request.
    ///
    /// Returns the request ID and a receiver for the result.
    pub fn create_pending(&self) -> (u64, oneshot::Receiver<CaptureResult>) {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let (tx, rx) = oneshot::channel();
        self.pending
            .write()
            .insert(id, PendingCapture { sender: tx });
        (id, rx)
    }

    /// Deliver a capture response.
    ///
    /// Returns `true` if the response was delivered, `false` if no matching request.
    pub fn deliver_response(&self, request_id: u64, result: CaptureResult) -> bool {
        // Extract pending request before processing to avoid holding lock
        let pending = self.pending.write().remove(&request_id);
        if let Some(pending) = pending {
            // Ignore send error (receiver dropped = timeout/cancelled)
            let _ = pending.sender.send(result);
            true
        } else {
            tracing::warn!("Received capture response for unknown request {request_id}");
            false
        }
    }

    /// Cancel a pending request (e.g., on timeout).
    ///
    /// Returns `true` if the request was found and cancelled.
    pub fn cancel(&self, request_id: u64) -> bool {
        self.pending.write().remove(&request_id).is_some()
    }

    /// Get the number of pending requests.
    #[must_use]
    pub fn pending_count(&self) -> usize {
        self.pending.read().len()
    }
}

impl Default for CaptureTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for CaptureTracker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CaptureTracker")
            .field("pending_count", &self.pending_count())
            .finish()
    }
}

/// Wait for a capture response with timeout.
///
/// # Arguments
///
/// * `rx` - The receiver for the capture result
///
/// # Errors
///
/// Returns `CaptureError::Timeout` if the timeout expires.
/// Returns `CaptureError::Disconnected` if the sender was dropped.
pub async fn wait_for_capture(
    rx: oneshot::Receiver<CaptureResult>,
) -> Result<CaptureResult, CaptureError> {
    match tokio::time::timeout(Duration::from_secs(CAPTURE_TIMEOUT_SECS), rx).await {
        Ok(Ok(result)) => Ok(result),
        Ok(Err(_)) => Err(CaptureError::Disconnected),
        Err(_) => Err(CaptureError::Timeout),
    }
}

#[cfg(test)]
#[path = "capture_tests.rs"]
mod tests;
