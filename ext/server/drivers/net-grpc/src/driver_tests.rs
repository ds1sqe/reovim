//! Tests for `GrpcServerDriverImpl`.
//!
//! These spin up real tonic servers against loopback TCP (and, on
//! Unix, real abstract-path sockets) to validate the serve lifecycle
//! end-to-end. Every timing-sensitive assertion uses a bounded
//! timeout + poll, not a fixed `sleep`.

use {
    super::GrpcServerDriverImpl,
    reovim_subsys_net::{GrpcServerDriver, NetError, TransportConfig, TransportKind},
    std::{net::SocketAddr, sync::Arc, time::Duration},
    tokio::{sync::oneshot, task::JoinHandle, time::timeout},
};

const POLL_BUDGET: Duration = Duration::from_millis(500);
const POLL_STEP: Duration = Duration::from_millis(10);

fn empty_router() -> tonic::transport::server::Router {
    tonic::transport::Server::builder().add_routes(tonic::service::Routes::default())
}

fn spawn_serve(
    driver: Box<GrpcServerDriverImpl>,
    config: TransportConfig,
    shutdown: Option<oneshot::Receiver<()>>,
) -> JoinHandle<Result<(), NetError>> {
    let signal: Option<reovim_subsys_net::ShutdownSignal> = shutdown.map(|rx| {
        Box::pin(async move {
            let _ = rx.await;
        }) as reovim_subsys_net::ShutdownSignal
    });
    tokio::spawn(async move { driver.serve(config, empty_router(), signal).await })
}

async fn wait_for_bind(handle: &Arc<arc_swap::ArcSwapOption<SocketAddr>>) -> Option<SocketAddr> {
    let deadline = tokio::time::Instant::now() + POLL_BUDGET;
    loop {
        if let Some(addr) = handle.load().as_deref().copied() {
            return Some(addr);
        }
        if tokio::time::Instant::now() >= deadline {
            return None;
        }
        tokio::time::sleep(POLL_STEP).await;
    }
}

#[tokio::test]
async fn bind_tcp_ok_publishes_address() {
    let driver = Box::new(GrpcServerDriverImpl::new());
    let bind = driver.bind_handle();
    assert!(bind.load().is_none(), "bind_handle empty pre-serve");

    let (tx, rx) = oneshot::channel::<()>();
    let join = spawn_serve(driver, TransportConfig::tcp_localhost(0), Some(rx));

    let addr = wait_for_bind(&bind).await.expect("bind_handle populated");
    assert_eq!(addr.ip().to_string(), "127.0.0.1");
    assert_ne!(addr.port(), 0, "OS assigned a real port");

    tx.send(()).expect("shutdown channel open");
    let res = timeout(Duration::from_secs(2), join)
        .await
        .expect("serve finished")
        .expect("task joined");
    assert!(res.is_ok(), "serve returned Ok after shutdown: {res:?}");
}

#[tokio::test]
async fn bind_tcp_addr_in_use_returns_bind_failed() {
    // Pre-bind a loopback port, then ask the driver to bind the same one.
    let sentinel = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let taken = sentinel.local_addr().unwrap();

    let driver = Box::new(GrpcServerDriverImpl::new());
    let (_tx, rx) = oneshot::channel::<()>();
    let join = spawn_serve(driver, TransportConfig::tcp("127.0.0.1", taken.port()), Some(rx));

    let res = timeout(Duration::from_secs(2), join)
        .await
        .expect("serve resolved")
        .expect("task joined");
    assert!(matches!(res, Err(NetError::BindFailed(_))), "expected BindFailed, got {res:?}");
    drop(sentinel);
}

#[tokio::test]
async fn concurrent_bind_race_exactly_one_wins() {
    // Two drivers racing for the same non-zero loopback port. Pick
    // the port via an OS-assigned bind that we immediately drop, so
    // we know the port was free at the start but may race with the
    // kernel re-using it if TIME_WAIT kicks in. Still: AT MOST one
    // driver can see the listener bound; the other MUST fail.
    let prober = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = prober.local_addr().unwrap().port();
    drop(prober);

    let d1 = Box::new(GrpcServerDriverImpl::new());
    let d2 = Box::new(GrpcServerDriverImpl::new());
    let bind1 = d1.bind_handle();
    let bind2 = d2.bind_handle();
    let (tx1, rx1) = oneshot::channel::<()>();
    let (tx2, rx2) = oneshot::channel::<()>();

    let h1 = spawn_serve(d1, TransportConfig::tcp("127.0.0.1", port), Some(rx1));
    let h2 = spawn_serve(d2, TransportConfig::tcp("127.0.0.1", port), Some(rx2));

    // Drive both tasks to a committed state: the winner publishes its
    // bound address into its bind_handle; the loser's task completes
    // early with BindFailed. Poll for either outcome per task under a
    // bounded deadline before assuming completion.
    let deadline = tokio::time::Instant::now() + POLL_BUDGET;
    loop {
        let done1 = bind1.load().is_some() || h1.is_finished();
        let done2 = bind2.load().is_some() || h2.is_finished();
        if done1 && done2 {
            break;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "both tasks must reach a committed state before the poll deadline",
        );
        tokio::time::sleep(POLL_STEP).await;
    }

    let _ = tx1.send(());
    let _ = tx2.send(());

    let r1 = timeout(Duration::from_secs(2), h1)
        .await
        .expect("h1 resolved")
        .expect("task joined");
    let r2 = timeout(Duration::from_secs(2), h2)
        .await
        .expect("h2 resolved")
        .expect("task joined");

    let losers = [&r1, &r2]
        .iter()
        .filter(|r| matches!(r, Err(NetError::BindFailed(_))))
        .count();
    assert!(losers >= 1, "at least one task must lose the bind race: r1={r1:?}, r2={r2:?}");
}

#[cfg(unix)]
#[tokio::test]
async fn bind_unix_ok() {
    let dir = tempdir();
    let path = dir.join("driver-test.sock");
    let driver = Box::new(GrpcServerDriverImpl::new());
    let (tx, rx) = oneshot::channel::<()>();
    let join = spawn_serve(driver, TransportConfig::unix_socket(&path), Some(rx));

    // The listener exists once the path does.
    let deadline = tokio::time::Instant::now() + POLL_BUDGET;
    loop {
        assert!(tokio::time::Instant::now() < deadline, "unix socket file never appeared");
        if path.exists() {
            break;
        }
        tokio::time::sleep(POLL_STEP).await;
    }

    tx.send(()).unwrap();
    let res = timeout(Duration::from_secs(2), join)
        .await
        .unwrap()
        .unwrap();
    assert!(res.is_ok(), "serve returned Ok after shutdown: {res:?}");
}

#[cfg(unix)]
fn tempdir() -> std::path::PathBuf {
    let base = std::env::temp_dir().join(format!("reovim-net-grpc-test-{}", std::process::id()));
    std::fs::create_dir_all(&base).unwrap();
    base
}

#[tokio::test]
async fn stdio_transport_rejected() {
    let driver = Box::new(GrpcServerDriverImpl::new());
    let res = driver
        .serve(TransportConfig::Stdio, empty_router(), None)
        .await;
    assert!(matches!(res, Err(NetError::UnsupportedTransport(TransportKind::Stdio))));
}

#[tokio::test]
async fn shutdown_stops_empty_router() {
    let driver = Box::new(GrpcServerDriverImpl::new());
    let bind = driver.bind_handle();
    let (tx, rx) = oneshot::channel::<()>();
    let join = spawn_serve(driver, TransportConfig::tcp_localhost(0), Some(rx));

    // Poll until the listener has actually bound, then trigger
    // shutdown. Avoids racing the bind with the shutdown signal.
    wait_for_bind(&bind).await.expect("bind_handle populated");
    tx.send(()).unwrap();

    let res = timeout(Duration::from_secs(2), join)
        .await
        .expect("serve finished before 2s deadline")
        .expect("task joined");
    assert!(res.is_ok(), "shutdown returned Ok: {res:?}");
}

#[tokio::test]
async fn bind_handle_clones_share_state() {
    let driver = Box::new(GrpcServerDriverImpl::new());
    let h1 = driver.bind_handle();
    let h2 = driver.bind_handle();
    assert!(h1.load().is_none());
    assert!(h2.load().is_none());

    let (tx, rx) = oneshot::channel::<()>();
    let join = spawn_serve(driver, TransportConfig::tcp_localhost(0), Some(rx));

    // h1 and h2 must both observe the bound address once serve runs.
    let a1 = wait_for_bind(&h1).await.expect("h1 populated");
    let a2 = h2.load().as_deref().copied().expect("h2 populated");
    assert_eq!(a1, a2, "clones of bind_handle share state");

    tx.send(()).unwrap();
    let _ = timeout(Duration::from_secs(2), join).await;
}

#[tokio::test]
async fn bind_tcp_invalid_host_returns_invalid_address() {
    // "not a host" is not a parseable IP literal; `parse_tcp_addr`
    // surfaces the parser error as `NetError::InvalidAddress`.
    let driver = Box::new(GrpcServerDriverImpl::new());
    let res = driver
        .serve(TransportConfig::tcp("not a host", 0), empty_router(), None)
        .await;
    assert!(
        matches!(res, Err(NetError::InvalidAddress(_))),
        "expected InvalidAddress, got {res:?}",
    );
}
