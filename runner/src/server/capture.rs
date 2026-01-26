//! TUI capture request tracking for `tui/capture` relay.
//!
//! Manages pending capture requests and correlates responses from TUI clients.
//!
//! # Flow
//!
//! 1. CLI sends `tui/capture` RPC request to server
//! 2. Server creates a pending request with a unique ID and `oneshot::Sender`
//! 3. Server sends `tui/capture-request` notification to TUI client
//! 4. TUI client processes request and sends `tui/capture-response` notification
//! 5. Server receives notification, finds pending request, sends result via channel
//! 6. Original handler receives result and responds to CLI

use std::{
    collections::HashMap,
    sync::atomic::{AtomicU64, Ordering},
    time::Duration,
};

use {
    reovim_arch::sync::RwLock,
    reovim_kernel::api::v1::Service,
    reovim_protocol::v1::{ScreenContentResult, notifications::CaptureResponsePayload},
    tokio::sync::oneshot,
};

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

/// Result type alias for capture operations.
pub type CaptureResult = Result<ScreenContentResult, CaptureError>;

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
    sender: oneshot::Sender<ScreenContentResult>,
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
    pub fn create_pending(&self) -> (u64, oneshot::Receiver<ScreenContentResult>) {
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
    pub fn deliver_response(&self, response: CaptureResponsePayload) -> bool {
        // Extract pending request before processing to avoid holding lock
        let pending = self.pending.write().remove(&response.request_id);
        if let Some(pending) = pending {
            // Ignore send error (receiver dropped = timeout/cancelled)
            let _ = pending.sender.send(response.result);
            true
        } else {
            tracing::warn!("Received capture response for unknown request {}", response.request_id);
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

// Implement Service so CaptureTracker can be stored in ServiceRegistry
impl Service for CaptureTracker {}

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
pub async fn wait_for_capture(rx: oneshot::Receiver<ScreenContentResult>) -> CaptureResult {
    match tokio::time::timeout(Duration::from_secs(CAPTURE_TIMEOUT_SECS), rx).await {
        Ok(Ok(result)) => Ok(result),
        Ok(Err(_)) => Err(CaptureError::Disconnected),
        Err(_) => Err(CaptureError::Timeout),
    }
}

#[cfg(test)]
mod tests {
    use reovim_protocol::v1::ScreenFormat;

    use super::*;

    #[test]
    fn test_capture_tracker_new() {
        let tracker = CaptureTracker::new();
        assert_eq!(tracker.pending_count(), 0);
    }

    #[test]
    fn test_create_pending() {
        let tracker = CaptureTracker::new();
        let (id1, _rx1) = tracker.create_pending();
        let (id2, _rx2) = tracker.create_pending();

        assert_eq!(id1, 1);
        assert_eq!(id2, 2);
        assert_eq!(tracker.pending_count(), 2);
    }

    #[test]
    fn test_deliver_response() {
        let tracker = CaptureTracker::new();
        let (id, mut rx) = tracker.create_pending();

        let response = CaptureResponsePayload::new(
            id,
            ScreenContentResult {
                width: 80,
                height: 24,
                format: ScreenFormat::PlainText,
                content: "test".to_string(),
            },
        );

        assert!(tracker.deliver_response(response));
        assert_eq!(tracker.pending_count(), 0);

        // Should have received the result
        assert!(rx.try_recv().is_ok());
    }

    #[test]
    fn test_deliver_response_unknown_id() {
        let tracker = CaptureTracker::new();

        let response = CaptureResponsePayload::new(
            999,
            ScreenContentResult {
                width: 80,
                height: 24,
                format: ScreenFormat::PlainText,
                content: "test".to_string(),
            },
        );

        assert!(!tracker.deliver_response(response));
    }

    #[test]
    fn test_cancel() {
        let tracker = CaptureTracker::new();
        let (id, _rx) = tracker.create_pending();

        assert!(tracker.cancel(id));
        assert_eq!(tracker.pending_count(), 0);
    }

    #[test]
    fn test_cancel_unknown_id() {
        let tracker = CaptureTracker::new();
        assert!(!tracker.cancel(999));
    }

    #[test]
    fn test_capture_error_display() {
        assert!(CaptureError::NoTuiClient.to_string().contains("No TUI"));
        assert!(CaptureError::Timeout.to_string().contains("timeout"));
        assert!(
            CaptureError::Disconnected
                .to_string()
                .contains("disconnected")
        );
    }
}
