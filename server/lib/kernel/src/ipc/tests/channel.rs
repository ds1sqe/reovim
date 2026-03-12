use {super::*, std::thread, std::time::Duration};

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

// === Unbounded try_recv disconnected ===

#[test]
fn test_unbounded_try_recv_disconnected() {
    let (tx, rx) = channel::<i32>();
    drop(tx);
    assert_eq!(rx.try_recv(), Err(TryRecvError::Disconnected));
}
