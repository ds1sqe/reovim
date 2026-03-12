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
