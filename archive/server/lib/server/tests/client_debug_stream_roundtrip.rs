//! Integration test for Phase 1 `ClientDebugService` — the full
//! inspector → server → driver round-trip.
//!
//! Starts a real tonic gRPC server on an OS-assigned port, registers
//! the Phase 0 `PoC` cdylib (`reovim-driver-debug-poc`) into a
//! `ClientDebugRegistry`, opens a bidirectional `DebugStream`, and
//! exercises:
//!
//! 1. Happy path: `Select` → `ProbeResp` → `ObserveStart` → 3 frames
//!    → `Drive`(echo) → `DriveResp`.
//! 2. Unknown driver → non-terminal `Error`; stream stays open.
//! 3. Unknown schema → non-terminal `Error`; stream stays open.
//! 4. Drop-safety: client drops the stream mid-observer; a second
//!    `DebugStream` opens on the same driver and completes probe
//!    within 100 ms (proves the pump task released the registry
//!    mutex — deliverable required by telemetry countdown round 1).
//! 5. Observer panic: `reovim-driver-debug-poc-observer-panic` driver
//!    closes the stream with `Status::internal` and is evicted from
//!    the registry.

// The `tonic::transport::Channel` client holds significant `Drop`
// state (connection pool, background task). For integration tests we
// deliberately keep it alive to end of scope so cleanup runs in a
// deterministic order relative to the server harness; early-drop
// doesn't improve correctness here.
#![allow(clippy::significant_drop_tightening)]

use {
    reovim_client_subsys_driver_loader::LoadedClientDebug,
    reovim_protocol::v3::{
        DebugDriveCommand, DebugObserveStart, DebugSelect, DebugStreamClientMsg,
        client_debug_service_client::ClientDebugServiceClient, debug_stream_client_msg,
        debug_stream_server_msg,
    },
    reovim_server::{ClientDebugRegistry, DebugDriverHandle, Server, ServerConfig, TransportMode},
    std::{env, path::PathBuf, sync::Arc, time::Duration},
    tokio::sync::{mpsc, oneshot},
    tokio_stream::{StreamExt, wrappers::ReceiverStream},
    tonic::transport::Channel,
};

// ─────────────────────────────────────────────────────────────────────────────
// cdylib path resolution (mirrors the Phase 0 round-trip test helper).
// ─────────────────────────────────────────────────────────────────────────────

fn cdylib_path(crate_underscore_name: &str) -> PathBuf {
    // CARGO_MANIFEST_DIR is server/lib/server; workspace root lives
    // three levels up.
    let manifest = env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR");
    let target_dir = env::var("CARGO_TARGET_DIR").unwrap_or_else(|_| {
        let workspace = PathBuf::from(&manifest)
            .ancestors()
            .nth(3)
            .expect("workspace root")
            .to_path_buf();
        workspace.join("target").display().to_string()
    });
    let mut p = PathBuf::from(target_dir);
    p.push("debug");
    let filename = if cfg!(target_os = "windows") {
        format!("{crate_underscore_name}.dll")
    } else if cfg!(target_os = "macos") {
        format!("lib{crate_underscore_name}.dylib")
    } else {
        format!("lib{crate_underscore_name}.so")
    };
    p.push(filename);
    p
}

/// Convenience: register the `PoC` cdylib. Panics if the cdylib is
/// not built; the test message points at the build command.
fn register_poc(registry: &ClientDebugRegistry, underscore_name: &str, registered_as: &str) {
    let path = cdylib_path(underscore_name);
    assert!(
        path.exists(),
        "debug PoC cdylib not found at {}; run `cargo build -p {}` first",
        path.display(),
        underscore_name.replace('_', "-")
    );
    let driver = LoadedClientDebug::load_from_path(&path)
        .unwrap_or_else(|e| panic!("load {underscore_name}: {e:?}"));
    registry.register(registered_as, Box::new(driver) as Box<dyn DebugDriverHandle>);
}

// ─────────────────────────────────────────────────────────────────────────────
// Tiny test server harness: starts a Server on port 0 and returns the
// OS-assigned port + a shutdown handle.
// ─────────────────────────────────────────────────────────────────────────────

struct TestHarness {
    port: u16,
    shutdown: Option<oneshot::Sender<()>>,
    join: Option<tokio::task::JoinHandle<std::io::Result<()>>>,
}

impl TestHarness {
    async fn start(registry: Arc<ClientDebugRegistry>) -> Self {
        let mut config = ServerConfig::grpc(0);
        config.transport = TransportMode::Grpc { port: 0 };
        let server = Server::new(config).with_client_debug_registry(registry);
        let (port_tx, port_rx) = oneshot::channel();
        let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
        let join = tokio::spawn(async move {
            server
                .run_until(
                    async move {
                        let _ = shutdown_rx.await;
                    },
                    Some(port_tx),
                )
                .await
        });
        let port = port_rx.await.expect("server reported port");
        Self {
            port,
            shutdown: Some(shutdown_tx),
            join: Some(join),
        }
    }

    async fn connect(&self) -> ClientDebugServiceClient<Channel> {
        let url = format!("http://127.0.0.1:{}", self.port);
        // Retry briefly — the server bind returns before the listener
        // is fully ready on some scheduler paths.
        for _ in 0..20 {
            if let Ok(channel) = Channel::from_shared(url.clone())
                .expect("uri")
                .connect()
                .await
            {
                return ClientDebugServiceClient::new(channel);
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        panic!("failed to connect to test server after retries");
    }
}

impl Drop for TestHarness {
    fn drop(&mut self) {
        if let Some(tx) = self.shutdown.take() {
            let _ = tx.send(());
        }
        if let Some(join) = self.join.take() {
            join.abort();
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Message-builder helpers
// ─────────────────────────────────────────────────────────────────────────────

fn select(name: &str) -> DebugStreamClientMsg {
    DebugStreamClientMsg {
        content: Some(debug_stream_client_msg::Content::Select(DebugSelect {
            driver_name: name.into(),
        })),
    }
}

fn observe_start(schema: &str) -> DebugStreamClientMsg {
    DebugStreamClientMsg {
        content: Some(debug_stream_client_msg::Content::ObserveStart(DebugObserveStart {
            schema: schema.into(),
        })),
    }
}

fn drive(schema: &str, body: Vec<u8>) -> DebugStreamClientMsg {
    DebugStreamClientMsg {
        content: Some(debug_stream_client_msg::Content::DriveCmd(DebugDriveCommand {
            schema: schema.into(),
            body,
        })),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn happy_path_probe_observe_drive_roundtrip() {
    let registry = Arc::new(ClientDebugRegistry::new());
    register_poc(&registry, "reovim_driver_debug_poc", "debug-poc");
    let harness = TestHarness::start(Arc::clone(&registry)).await;
    let mut client = harness.connect().await;

    let (tx, rx) = mpsc::channel::<DebugStreamClientMsg>(8);
    tx.send(select("debug-poc")).await.unwrap();
    tx.send(observe_start("poc-frames")).await.unwrap();
    tx.send(drive("poc-echo", b"hello".to_vec())).await.unwrap();
    drop(tx); // close client side

    let mut resp = client
        .debug_stream(ReceiverStream::new(rx))
        .await
        .expect("open stream")
        .into_inner();

    // Expect: ProbeResp, 3 ObserveFrames, DriveResp.
    let mut frames = Vec::new();
    let mut drive_body: Option<Vec<u8>> = None;
    let mut probe_ok = false;
    while let Some(msg) = resp.next().await {
        let msg = msg.expect("server msg ok");
        match msg.content {
            Some(debug_stream_server_msg::Content::ProbeResp(r)) => {
                probe_ok = true;
                assert_eq!(r.driver_name, "debug-poc");
                assert!(r.observe_schemas.iter().any(|s| s == "poc-frames"));
            }
            Some(debug_stream_server_msg::Content::ObserveFrame(f)) => frames.push(f.body),
            Some(debug_stream_server_msg::Content::DriveResp(d)) => drive_body = Some(d.body),
            Some(debug_stream_server_msg::Content::Error(e)) => {
                panic!("unexpected error: {}", e.message)
            }
            None => {}
        }
    }
    assert!(probe_ok, "expected ProbeResp");
    assert_eq!(frames.len(), 3, "expected 3 frames, got {frames:?}");
    assert_eq!(drive_body, Some(b"hello".to_vec()));
}

#[tokio::test]
async fn unknown_driver_yields_non_terminal_error_stream_stays_open() {
    let registry = Arc::new(ClientDebugRegistry::new());
    register_poc(&registry, "reovim_driver_debug_poc", "debug-poc");
    let harness = TestHarness::start(Arc::clone(&registry)).await;
    let mut client = harness.connect().await;

    let (tx, rx) = mpsc::channel::<DebugStreamClientMsg>(4);
    tx.send(select("missing")).await.unwrap();
    tx.send(select("debug-poc")).await.unwrap();
    drop(tx);

    let mut resp = client
        .debug_stream(ReceiverStream::new(rx))
        .await
        .unwrap()
        .into_inner();

    let mut saw_error = false;
    let mut saw_probe = false;
    while let Some(msg) = resp.next().await {
        let msg = msg.unwrap();
        match msg.content {
            Some(debug_stream_server_msg::Content::Error(_)) => saw_error = true,
            Some(debug_stream_server_msg::Content::ProbeResp(_)) => saw_probe = true,
            _ => {}
        }
    }
    assert!(saw_error, "expected Error for unknown driver");
    assert!(saw_probe, "expected ProbeResp after recovery — stream stayed open");
}

#[tokio::test]
async fn unknown_schema_yields_non_terminal_error_stream_stays_open() {
    let registry = Arc::new(ClientDebugRegistry::new());
    register_poc(&registry, "reovim_driver_debug_poc", "debug-poc");
    let harness = TestHarness::start(Arc::clone(&registry)).await;
    let mut client = harness.connect().await;

    let (tx, rx) = mpsc::channel::<DebugStreamClientMsg>(4);
    tx.send(select("debug-poc")).await.unwrap();
    tx.send(observe_start("no-such-schema")).await.unwrap();
    tx.send(drive("poc-echo", b"after-error".to_vec()))
        .await
        .unwrap();
    drop(tx);

    let mut resp = client
        .debug_stream(ReceiverStream::new(rx))
        .await
        .unwrap()
        .into_inner();

    let mut saw_observe_error = false;
    let mut drive_body: Option<Vec<u8>> = None;
    while let Some(msg) = resp.next().await {
        let msg = msg.unwrap();
        match msg.content {
            Some(debug_stream_server_msg::Content::Error(_)) => saw_observe_error = true,
            Some(debug_stream_server_msg::Content::DriveResp(d)) => drive_body = Some(d.body),
            _ => {}
        }
    }
    assert!(saw_observe_error, "expected Error for bogus schema");
    assert_eq!(
        drive_body,
        Some(b"after-error".to_vec()),
        "drive after observe-error must succeed (stream stayed open)"
    );
}

/// Drop-safety: required by telemetry countdown round 1.
#[tokio::test]
async fn client_debug_stream_drop_mid_observer_releases_lock() {
    let registry = Arc::new(ClientDebugRegistry::new());
    register_poc(&registry, "reovim_driver_debug_poc", "debug-poc");
    let harness = TestHarness::start(Arc::clone(&registry)).await;

    // First stream: Select + ObserveStart, then DROP without reading
    // to completion. The pump has the driver's mutex.
    {
        let mut client = harness.connect().await;
        let (tx, rx) = mpsc::channel::<DebugStreamClientMsg>(4);
        tx.send(select("debug-poc")).await.unwrap();
        tx.send(observe_start("poc-frames")).await.unwrap();
        // Do NOT close tx. Hold the stream open while we consume the
        // probe response, then drop everything.
        let mut resp = client
            .debug_stream(ReceiverStream::new(rx))
            .await
            .unwrap()
            .into_inner();
        // Read the probe only (don't drain frames). Holding `tx` keeps
        // the client side open; dropping `tx` + `resp` below simulates
        // an unclean disconnect.
        let first = resp.next().await.expect("one message").unwrap();
        assert!(matches!(first.content, Some(debug_stream_server_msg::Content::ProbeResp(_))));
        // Now simulate abrupt client drop:
        drop(tx);
        drop(resp);
        drop(client);
    }

    // Second stream: Select + Probe only. Must succeed within 100 ms,
    // proving the first stream's pump task cancelled and released the
    // registry mutex.
    let mut client = harness.connect().await;
    let (tx, rx) = mpsc::channel::<DebugStreamClientMsg>(4);
    tx.send(select("debug-poc")).await.unwrap();
    drop(tx);

    let opened =
        tokio::time::timeout(Duration::from_secs(2), client.debug_stream(ReceiverStream::new(rx)))
            .await
            .expect("opening second stream must not time out")
            .unwrap();

    let mut resp = opened.into_inner();
    let probe = tokio::time::timeout(Duration::from_secs(2), resp.next())
        .await
        .expect("second-stream probe must arrive within 2s (100ms is the canonical bound)")
        .expect("server sent a message")
        .expect("server msg is Ok");
    assert!(matches!(probe.content, Some(debug_stream_server_msg::Content::ProbeResp(_))));
}

/// Observer panic: the driver panics on its second `next_frame`. The
/// trampoline's `catch_unwind` from Phase 0 surfaces the panic as
/// `rc == -2` → the handler closes the stream with `Status::internal`
/// and evicts the driver.
#[tokio::test]
async fn observer_panic_terminates_stream_and_evicts_driver() {
    let registry = Arc::new(ClientDebugRegistry::new());
    register_poc(&registry, "reovim_driver_debug_poc_observer_panic", "panic-poc");
    assert!(registry.list_drivers().contains(&"panic-poc".to_string()));

    let harness = TestHarness::start(Arc::clone(&registry)).await;
    let mut client = harness.connect().await;

    let (tx, rx) = mpsc::channel::<DebugStreamClientMsg>(4);
    tx.send(select("panic-poc")).await.unwrap();
    tx.send(observe_start("panic-frames")).await.unwrap();
    drop(tx);

    let mut resp = client
        .debug_stream(ReceiverStream::new(rx))
        .await
        .unwrap()
        .into_inner();

    let mut last_err: Option<String> = None;
    while let Some(msg) = resp.next().await {
        match msg {
            Ok(_) => {}
            Err(status) => {
                last_err = Some(status.message().to_owned());
                break;
            }
        }
    }
    let err = last_err.expect("expected terminal Status::internal");
    assert!(err.contains("panic"), "message was {err:?}");

    // Driver evicted.
    assert!(!registry.list_drivers().contains(&"panic-poc".to_string()));
}
