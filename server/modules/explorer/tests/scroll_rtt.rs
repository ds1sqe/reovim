//! Explorer scroll RTT (round-trip time) measurement.
//!
//! Integration tests that measure the real end-to-end latency from
//! `send_keys("j")` to receiving an `ExtensionUpdated` notification
//! back at the client.
//!
//! These are NOT Criterion benchmarks — each iteration takes milliseconds
//! (too slow for Criterion's microsecond expectations). Instead we collect
//! latency statistics manually and print them to stderr.
//!
//! # Prerequisites
//!
//! The `reovim` binary must be built before running:
//! ```bash
//! cargo build -p reovim-app
//! cargo test -p reovim-module-explorer --test scroll_rtt -- --nocapture
//! ```

use std::time::{Duration, Instant};

use {
    reovim_protocol::v2::{
        JoinRequest, SendKeysRequest, SubscribeRequest, input_service_client::InputServiceClient,
        notification::Payload, notification_service_client::NotificationServiceClient,
        presence_service_client::PresenceServiceClient,
    },
    reovim_testing::TestServerHarness,
    tonic::{Request, Streaming, transport::Channel},
};

/// Number of sequential RTT iterations.
const SEQUENTIAL_ITERATIONS: usize = 100;

/// Number of rapid-fire keys in burst test.
const BURST_SIZE: usize = 30;

/// Timeout for waiting for a single notification.
const NOTIFICATION_TIMEOUT: Duration = Duration::from_secs(5);

/// Settle time after toggling explorer on.
const SETTLE_TIME: Duration = Duration::from_millis(200);

/// Create a `tonic::Request` with the session token as metadata.
fn authed_request<T>(body: T, token: &str) -> Request<T> {
    let mut request = Request::new(body);
    request
        .metadata_mut()
        .insert("x-reovim-token", token.parse().expect("valid ASCII token"));
    request
}

/// Send keys via gRPC with authentication.
async fn send_keys(client: &mut InputServiceClient<Channel>, token: &str, keys: &str) {
    let request = authed_request(
        SendKeysRequest {
            keys: keys.to_string(),
        },
        token,
    );
    client
        .send_keys(request)
        .await
        .expect("send_keys should succeed");
}

/// Wait for the next `ExtensionUpdated` notification with `kind == "explorer"`.
///
/// Skips non-explorer notifications (`mode_changed`, `cursor_moved`, etc.).
async fn wait_explorer_notification(
    stream: &mut Streaming<reovim_protocol::v2::Notification>,
) -> reovim_protocol::v2::Notification {
    loop {
        let msg = tokio::time::timeout(NOTIFICATION_TIMEOUT, stream.message())
            .await
            .expect("notification should arrive within timeout")
            .expect("stream should not error")
            .expect("stream should not end");

        if matches!(
            msg.payload,
            Some(Payload::ExtensionUpdated(ref ext)) if ext.kind == "explorer"
        ) {
            return msg;
        }
    }
}

/// Drain all pending notifications (non-blocking).
///
/// Keeps reading until no notification arrives within 100ms.
async fn drain_notifications(stream: &mut Streaming<reovim_protocol::v2::Notification>) {
    while let Ok(Ok(Some(_))) =
        tokio::time::timeout(Duration::from_millis(100), stream.message()).await
    {}
}

/// Print latency statistics from a sorted list of durations.
fn print_stats(label: &str, latencies: &[Duration]) {
    let n = latencies.len();
    if n == 0 {
        eprintln!("{label}: no data");
        return;
    }

    let mean_us = latencies.iter().map(Duration::as_micros).sum::<u128>() / n as u128;
    let min_us = latencies[0].as_micros();
    let max_us = latencies[n - 1].as_micros();
    let p50_us = latencies[n / 2].as_micros();
    let p95_us = latencies[n * 95 / 100].as_micros();
    let p99_us = latencies[n * 99 / 100].as_micros();

    eprintln!("{label} ({n} iterations):");
    eprintln!(
        "  min: {min_us}us  mean: {mean_us}us  p50: {p50_us}us  \
         p95: {p95_us}us  p99: {p99_us}us  max: {max_us}us"
    );
}

/// Connect to the server, join presence, subscribe to notifications,
/// and toggle the explorer on.
///
/// Returns the input client, notification stream, and session token.
#[allow(clippy::significant_drop_tightening)]
async fn setup_explorer_client(
    port: u16,
) -> (
    InputServiceClient<Channel>,
    Streaming<reovim_protocol::v2::Notification>,
    String,
) {
    let addr = format!("http://127.0.0.1:{port}");
    let channel = Channel::from_shared(addr)
        .expect("valid URI")
        .connect()
        .await
        .expect("should connect to server");

    let mut input = InputServiceClient::new(channel.clone());
    let mut presence = PresenceServiceClient::new(channel.clone());
    let mut notification = NotificationServiceClient::new(channel);

    // Join presence to get session token
    let join_resp = presence
        .join(JoinRequest {
            client_type: "bench".into(),
            display_name: "rtt-bench".into(),
        })
        .await
        .expect("join should succeed")
        .into_inner();
    drop(presence);
    let token = join_resp.session_token;

    // Subscribe to extension_updated notifications
    let subscribe_req = authed_request(
        SubscribeRequest {
            event_types: vec!["extension_updated".into()],
        },
        &token,
    );
    let mut stream = notification
        .subscribe(subscribe_req)
        .await
        .expect("subscribe should succeed")
        .into_inner();
    drop(notification);

    // Toggle explorer on with <Space>e
    send_keys(&mut input, &token, "<Space>e").await;

    // Wait for the explorer to settle
    tokio::time::sleep(SETTLE_TIME).await;
    drain_notifications(&mut stream).await;

    (input, stream, token)
}

/// Measure sequential scroll RTT.
///
/// Sends "j" one at a time, waiting for each `ExtensionUpdated` notification
/// before sending the next. Reports per-key latency statistics.
#[tokio::test]
async fn measure_scroll_rtt() {
    let harness = TestServerHarness::spawn()
        .await
        .expect("server should start");

    let (mut input, mut stream, token) = setup_explorer_client(harness.port()).await;

    let mut latencies = Vec::with_capacity(SEQUENTIAL_ITERATIONS);

    for _ in 0..SEQUENTIAL_ITERATIONS {
        let start = Instant::now();
        send_keys(&mut input, &token, "j").await;
        wait_explorer_notification(&mut stream).await;
        latencies.push(start.elapsed());
    }

    latencies.sort();
    print_stats("Explorer scroll RTT (sequential)", &latencies);

    // Measure payload size from one more notification
    send_keys(&mut input, &token, "j").await;
    let notif = wait_explorer_notification(&mut stream).await;
    if let Some(Payload::ExtensionUpdated(ext)) = notif.payload {
        eprintln!("  payload size: {} bytes", ext.data.len());
    }
}

/// Measure burst scroll RTT.
///
/// Sends `BURST_SIZE` "j" keys as fast as possible (simulating a held key),
/// then waits for all notifications to arrive. Reports total time and
/// per-key average.
#[tokio::test]
async fn measure_scroll_burst_rtt() {
    let harness = TestServerHarness::spawn()
        .await
        .expect("server should start");

    let (mut input, mut stream, token) = setup_explorer_client(harness.port()).await;

    let start = Instant::now();

    // Send all keys as fast as possible
    for _ in 0..BURST_SIZE {
        send_keys(&mut input, &token, "j").await;
    }

    // Wait for all notifications
    let mut received = 0;
    while received < BURST_SIZE {
        wait_explorer_notification(&mut stream).await;
        received += 1;
    }

    let total = start.elapsed();
    eprintln!("Explorer scroll RTT (burst, {BURST_SIZE} keys):");
    eprintln!(
        "  total: {}ms  per-key: {}us",
        total.as_millis(),
        total.as_micros() / BURST_SIZE as u128
    );
}
