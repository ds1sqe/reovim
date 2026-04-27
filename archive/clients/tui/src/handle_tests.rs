use super::*;

#[test]
fn test_tui_handle_error_display() {
    let err = TuiHandleError::NotRunning;
    assert_eq!(err.to_string(), "TUI event loop not running");

    let err = TuiHandleError::Timeout;
    assert_eq!(err.to_string(), "Timeout waiting for condition");
}

#[tokio::test]
async fn test_handle_stop_on_closed_channel() {
    let (tx, rx) = mpsc::channel(1);
    let handle = TuiHandle::new(tx);

    // Drop the receiver
    drop(rx);

    // stop() should not panic
    handle.stop().await;
}

#[tokio::test]
async fn test_handle_send_keys_not_running() {
    let (tx, rx) = mpsc::channel(1);
    let handle = TuiHandle::new(tx);

    drop(rx);

    let result = handle.send_keys("ihello").await;
    assert!(result.is_err());
}

/// Test `wait_for` succeeds when the predicate is met immediately.
///
/// This exercises lines 181-184: the while loop body, capture call,
/// and the `if predicate(&frame)` true branch returning `Ok(frame)`.
#[tokio::test]
async fn test_wait_for_succeeds_when_predicate_met() {
    let (tx, mut rx) = mpsc::channel::<TuiInput>(8);
    let handle = TuiHandle::new(tx);

    // Spawn a task that responds to Capture requests with "hello world"
    tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            if let TuiInput::Control(ControlRequest::Capture { response, .. }) = msg {
                let _ = response.send("hello world".to_string());
            }
        }
    });

    let result = handle
        .wait_for(Duration::from_millis(500), |frame| frame.contains("hello"))
        .await;

    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "hello world");
}

/// Test `wait_for` loops when the predicate is not yet met, then succeeds.
///
/// This exercises the loop continuing when `predicate(&frame)` returns false,
/// then eventually returning `Ok(frame)` when the predicate becomes true.
#[tokio::test]
async fn test_wait_for_loops_then_succeeds() {
    use std::sync::{Arc, Mutex};

    let (tx, mut rx) = mpsc::channel::<TuiInput>(8);
    let handle = TuiHandle::new(tx);

    // Counter: first call returns "not ready", second returns "ready"
    let call_count = Arc::new(Mutex::new(0u32));
    let call_count_clone = Arc::clone(&call_count);

    tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            if let TuiInput::Control(ControlRequest::Capture { response, .. }) = msg {
                let mut count = call_count_clone.lock().unwrap();
                *count += 1;
                let frame = if *count == 1 {
                    "not ready".to_string()
                } else {
                    "ready content".to_string()
                };
                drop(count);
                let _ = response.send(frame);
            }
        }
    });

    let result = handle
        .wait_for(Duration::from_millis(500), |frame| frame.contains("ready content"))
        .await;

    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "ready content");
}

/// Test `wait_for` returns `Timeout` when the predicate is never met.
#[tokio::test]
async fn test_wait_for_timeout() {
    let (tx, mut rx) = mpsc::channel::<TuiInput>(8);
    let handle = TuiHandle::new(tx);

    // Always respond with "nothing" (predicate never matches)
    tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            if let TuiInput::Control(ControlRequest::Capture { response, .. }) = msg {
                let _ = response.send("nothing".to_string());
            }
        }
    });

    let result = handle
        .wait_for(Duration::from_millis(25), |frame| frame.contains("never"))
        .await;

    assert!(matches!(result, Err(TuiHandleError::Timeout)));
}

/// Test `wait_for` returns error when channel closes mid-loop.
#[tokio::test]
async fn test_wait_for_channel_closed() {
    let (tx, rx) = mpsc::channel::<TuiInput>(1);
    let handle = TuiHandle::new(tx);

    // Close the receiver immediately so capture fails
    drop(rx);

    let result = handle.wait_for(Duration::from_millis(100), |_| false).await;

    assert!(matches!(result, Err(TuiHandleError::NotRunning)));
}

/// Test `TuiHandleError` implements `std::error::Error`.
#[test]
fn test_tui_handle_error_is_error() {
    let err: Box<dyn std::error::Error> = Box::new(TuiHandleError::NotRunning);
    assert!(!err.to_string().is_empty());
}
