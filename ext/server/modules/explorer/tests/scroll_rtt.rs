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
//! cargo build -p reovim
//! cargo test -p reovim-module-explorer --test scroll_rtt -- --nocapture
//! ```
//!
//! # Design (#695)
//!
//! The server's dedup cache suppresses `ExtensionUpdated` notifications when
//! the JSON payload is identical to the previous emission. We use alternating
//! `j`/`k` keys so `cursorIndex` oscillates and the JSON always differs.
//! The test subscribes to ALL notification types (no server filter) because
//! server-side event type filtering can interact with stream state.

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
const SEQUENTIAL_ITERATIONS: usize = 50;

/// Number of rapid-fire keys in burst test.
const BURST_SIZE: usize = 20;

/// Timeout for waiting for a single notification.
const NOTIFICATION_TIMEOUT: Duration = Duration::from_secs(5);

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
///
/// # Setup (#695)
///
/// Subscribes to ALL notification types and consumes the initial toggle
/// notification + a warmup `j` to ensure the stream is healthy. The warmup
/// also moves the cursor to index 1, so the loop can alternate `j`/`k`
/// starting from a non-zero position.
#[allow(clippy::significant_drop_tightening)]
async fn setup_explorer_client(
    port: u16,
) -> (
    InputServiceClient<Channel>,
    Streaming<reovim_protocol::v2::Notification>,
    String,
) {
    let addr = format!("http://127.0.0.1:{port}");
    let channel = Channel::from_shared(addr.clone())
        .expect("valid URI")
        .connect()
        .await
        .expect("should connect to server");

    let mut input = InputServiceClient::new(channel.clone());
    let mut presence = PresenceServiceClient::new(channel);
    let mut notification = NotificationServiceClient::new(
        Channel::from_shared(addr)
            .expect("valid URI")
            .connect()
            .await
            .expect("should connect notification channel"),
    );

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

    // Subscribe to ALL notifications (no server-side filter).
    let subscribe_req = authed_request(
        SubscribeRequest {
            event_types: vec![],
        },
        &token,
    );
    let mut stream = notification
        .subscribe(subscribe_req)
        .await
        .expect("subscribe should succeed")
        .into_inner();
    drop(notification);

    // Toggle explorer on and consume the activation notification.
    send_keys(&mut input, &token, "<Space>e").await;
    wait_explorer_notification(&mut stream).await;

    // Warmup: move cursor to index 1 so j/k alternation works.
    send_keys(&mut input, &token, "j").await;
    wait_explorer_notification(&mut stream).await;

    (input, stream, token)
}

/// Measure sequential scroll RTT.
///
/// Alternates `j` and `k` so `cursorIndex` oscillates between 1 and 2,
/// producing unique JSON on every keystroke (avoiding dedup suppression).
#[tokio::test]
async fn measure_scroll_rtt() {
    let harness = TestServerHarness::spawn()
        .await
        .expect("server should start");

    let (mut input, mut stream, token) = setup_explorer_client(harness.port()).await;

    let mut latencies = Vec::with_capacity(SEQUENTIAL_ITERATIONS);
    let keys = ["j", "k"];

    for i in 0..SEQUENTIAL_ITERATIONS {
        let key = keys[i % 2];
        let start = Instant::now();
        send_keys(&mut input, &token, key).await;
        wait_explorer_notification(&mut stream).await;
        latencies.push(start.elapsed());
    }

    latencies.sort();
    print_stats("Explorer scroll RTT (sequential j/k)", &latencies);

    // Measure payload size from one more notification
    send_keys(&mut input, &token, "j").await;
    let notif = wait_explorer_notification(&mut stream).await;
    if let Some(Payload::ExtensionUpdated(ext)) = notif.payload {
        eprintln!("  payload size: {} bytes", ext.data.len());
    }
}

/// Measure burst scroll RTT.
///
/// Sends `BURST_SIZE` alternating `j`/`k` keys as fast as possible
/// (simulating rapid scrolling), then waits for all notifications.
#[tokio::test]
async fn measure_scroll_burst_rtt() {
    let harness = TestServerHarness::spawn()
        .await
        .expect("server should start");

    let (mut input, mut stream, token) = setup_explorer_client(harness.port()).await;

    let start = Instant::now();
    let keys = ["j", "k"];

    // Send all keys as fast as possible
    for i in 0..BURST_SIZE {
        send_keys(&mut input, &token, keys[i % 2]).await;
    }

    // Wait for all notifications
    let mut received = 0;
    while received < BURST_SIZE {
        wait_explorer_notification(&mut stream).await;
        received += 1;
    }

    let total = start.elapsed();
    eprintln!("Explorer scroll RTT (burst j/k, {BURST_SIZE} keys):");
    eprintln!(
        "  total: {}ms  per-key: {}us",
        total.as_millis(),
        total.as_micros() / BURST_SIZE as u128
    );
}
