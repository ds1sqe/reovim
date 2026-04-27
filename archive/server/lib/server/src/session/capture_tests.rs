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

    let result = CaptureResult {
        width: 80,
        height: 24,
        format: "plain_text".to_string(),
        content: "test".to_string(),
    };

    assert!(tracker.deliver_response(id, result));
    assert_eq!(tracker.pending_count(), 0);

    // Should have received the result
    assert!(rx.try_recv().is_ok());
}

#[test]
fn test_deliver_response_unknown_id() {
    let tracker = CaptureTracker::new();

    let result = CaptureResult {
        width: 80,
        height: 24,
        format: "plain_text".to_string(),
        content: "test".to_string(),
    };

    assert!(!tracker.deliver_response(999, result));
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

#[test]
fn test_capture_error_invalid_response() {
    let err = CaptureError::InvalidResponse("bad data".into());
    assert!(err.to_string().contains("bad data"));
}

#[test]
fn test_capture_error_debug() {
    let err = CaptureError::Timeout;
    let debug = format!("{err:?}");
    assert!(debug.contains("Timeout"));
}

#[test]
fn test_capture_error_is_error() {
    let err = CaptureError::NoTuiClient;
    // Verify it implements std::error::Error
    let _: &dyn std::error::Error = &err;
}

#[test]
fn test_capture_tracker_default() {
    let tracker = CaptureTracker::default();
    assert_eq!(tracker.pending_count(), 0);
}

#[test]
fn test_capture_tracker_debug() {
    let tracker = CaptureTracker::new();
    let debug = format!("{tracker:?}");
    assert!(debug.contains("CaptureTracker"));
    assert!(debug.contains("pending_count"));
}

#[test]
fn test_capture_result_clone() {
    let result = CaptureResult {
        width: 80,
        height: 24,
        format: "ansi".to_string(),
        content: "hello".to_string(),
    };
    #[allow(clippy::redundant_clone)]
    let cloned = result.clone();
    assert_eq!(cloned.width, 80);
    assert_eq!(cloned.content, "hello");
}

#[test]
fn test_deliver_response_after_receiver_dropped() {
    let tracker = CaptureTracker::new();
    let (id, rx) = tracker.create_pending();
    drop(rx); // Drop receiver

    let result = CaptureResult {
        width: 80,
        height: 24,
        format: "plain_text".to_string(),
        content: "test".to_string(),
    };
    // Should still return true (request was found), even though send fails
    assert!(tracker.deliver_response(id, result));
}

#[tokio::test]
async fn test_wait_for_capture_success() {
    let (tx, rx) = oneshot::channel();
    let result = CaptureResult {
        width: 80,
        height: 24,
        format: "plain_text".to_string(),
        content: "captured".to_string(),
    };
    tx.send(result).unwrap();

    let received = wait_for_capture(rx).await;
    assert!(received.is_ok());
    assert_eq!(received.unwrap().content, "captured");
}

#[tokio::test]
async fn test_wait_for_capture_disconnected() {
    let (tx, rx) = oneshot::channel::<CaptureResult>();
    drop(tx); // Sender dropped

    let result = wait_for_capture(rx).await;
    assert!(matches!(result, Err(CaptureError::Disconnected)));
}
