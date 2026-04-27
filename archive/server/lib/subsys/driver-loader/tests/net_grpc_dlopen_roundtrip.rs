//! End-to-end SP02 Phase 5 net-grpc driver-ABI lifecycle round-trip.
//!
//! `dlopen`s the `reovim-driver-net-grpc` cdylib produced by the
//! workspace, validates the vtable header, constructs a driver
//! instance, drives a real `serve` call against `127.0.0.1:0` (OS-
//! assigned port) on a `tokio::task::spawn_blocking` worker, waits on
//! the bind-ready pipe, verifies the listener is reachable, signals
//! shutdown via the shutdown pipe, awaits clean serve return, and
//! drops the loader (which calls vtable.shutdown + vtable.destroy
//! through the FFI boundary).
//!
//! Phase 5 scope: lifecycle only; no descriptor marshalling.
//! Non-empty descriptor lists are rejected by `LoadedNetGrpc::serve`
//! per the `SP02b` deferral (`tmp/deferral-draft-sp02b-dispatch-call.md`),
//! so this test passes an empty `Vec<ServiceDescriptor>`.

#![cfg(unix)]
#![allow(unsafe_code)] // pipe2 + raw fd manipulation

use {
    reovim_subsys_driver_loader::LoadedNetGrpc,
    reovim_subsys_net::{TransportConfig, abi::ShutdownFd},
    std::{
        env,
        os::fd::{FromRawFd, OwnedFd},
        path::PathBuf,
        sync::{
            Arc,
            atomic::{AtomicU16, Ordering},
        },
        time::Duration,
    },
    tokio::{io::unix::AsyncFd, net::TcpStream, time::timeout},
};

/// Resolve the workspace target dir → `target/debug/lib<name>.so`.
fn cdylib_path(crate_underscore_name: &str) -> PathBuf {
    let manifest = env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR");
    let target_dir = env::var("CARGO_TARGET_DIR").unwrap_or_else(|_| {
        // crate → driver-loader → subsys → lib → server → workspace-root
        let workspace = PathBuf::from(&manifest)
            .ancestors()
            .nth(4)
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

/// Create a (read, write) pipe pair via `libc::pipe2` with `O_CLOEXEC`.
/// Returns owned fds wrapped in `OwnedFd` so the test process owns the
/// close. Returns the raw read fd separately so we can hand it as a
/// `ShutdownFd` to the driver — the driver borrows the fd, so the
/// `OwnedFd` we keep alongside guarantees the fd stays open until the
/// test drops the wrapper.
fn make_pipe() -> (OwnedFd, OwnedFd) {
    let mut fds: [libc::c_int; 2] = [-1; 2];
    // SAFETY: pipe2 writes two valid fds into the pair on success.
    let rc = unsafe { libc::pipe2(fds.as_mut_ptr(), libc::O_CLOEXEC) };
    assert_eq!(rc, 0, "pipe2 failed: errno={}", std::io::Error::last_os_error());
    // SAFETY: fds are freshly allocated by the kernel; ownership now
    // moves to the OwnedFd values which will close them on drop.
    let read_end = unsafe { OwnedFd::from_raw_fd(fds[0]) };
    let write_end = unsafe { OwnedFd::from_raw_fd(fds[1]) };
    (read_end, write_end)
}

#[tokio::test]
async fn lifecycle_roundtrip_empty_descriptors() {
    use std::os::fd::AsRawFd;

    let path = cdylib_path("reovim_driver_net_grpc");
    assert!(
        path.exists(),
        "net-grpc cdylib not found at {}; run \
         `cargo build -p reovim-driver-net-grpc` first",
        path.display()
    );

    // Two pipes: one for shutdown signalling (host writes to wake the
    // driver), one for bind-ready signalling (driver writes to wake
    // the host).
    let (shutdown_r, shutdown_w) = make_pipe();
    let (bind_ready_r, bind_ready_w) = make_pipe();

    let shutdown_fd = ShutdownFd(shutdown_r.as_raw_fd());
    let bind_ready_fd = ShutdownFd(bind_ready_w.as_raw_fd());

    // Host-allocated, leaked atomic for the driver to write the bound
    // port into. `Box::leak` gives us a `&'static AtomicU16` per the
    // serve contract (SP02 Phase 1 fd-arch round-1 #2).
    let writeback: &'static AtomicU16 = Box::leak(Box::new(AtomicU16::new(0)));

    let driver = Arc::new(LoadedNetGrpc::load_from_path(&path).expect("load net-grpc cdylib"));
    let driver_for_serve = Arc::clone(&driver);

    let serve_handle = tokio::task::spawn_blocking(move || {
        driver_for_serve.serve(
            &TransportConfig::tcp_localhost(0),
            Vec::new(),
            shutdown_fd,
            bind_ready_fd,
            writeback,
        )
    });

    // Wait for the bind-ready pipe to become readable. Wrap the read
    // end as `AsyncFd` for tokio integration.
    let bind_ready_async = AsyncFd::new(bind_ready_r).expect("AsyncFd::new(bind_ready_r)");
    let _readable = timeout(Duration::from_secs(5), bind_ready_async.readable())
        .await
        .expect("bind-ready timed out — driver did not signal within 5s")
        .expect("bind-ready readable error");

    // Read the bound port from the leaked atomic.
    let port = writeback.load(Ordering::Acquire);
    assert_ne!(port, 0, "driver did not write a non-zero port");

    // Verify the listener is live by completing a TCP connect.
    TcpStream::connect(format!("127.0.0.1:{port}"))
        .await
        .expect("listener should accept connections");

    // Signal shutdown by writing one byte to the writable end of the
    // shutdown pipe.
    let written =
        unsafe { libc::write(shutdown_w.as_raw_fd(), b"x".as_ptr().cast::<libc::c_void>(), 1) };
    assert_eq!(written, 1, "write to shutdown pipe failed");

    // Await the spawn_blocking handle and assert clean shutdown.
    let serve_result = timeout(Duration::from_secs(5), serve_handle)
        .await
        .expect("serve handle did not complete within 5s")
        .expect("spawn_blocking task panicked");
    serve_result.expect("serve returned a NetError");

    // Drop the loader — vtable.shutdown + vtable.destroy run through
    // the FFI boundary. Reaching this assertion without panic means
    // both trampolines completed cleanly.
    drop(driver);

    // Drop the still-held pipe ends explicitly to make the cleanup
    // order obvious to readers.
    drop(shutdown_w);
}

#[tokio::test]
async fn from_path_scan_stages_real_cdylib_and_returns_one_ok_entry() {
    use std::fs;

    let src = cdylib_path("reovim_driver_net_grpc");
    assert!(
        src.exists(),
        "net-grpc cdylib not found at {}; run \
         `cargo build -p reovim-driver-net-grpc` first",
        src.display()
    );

    // Stage the real driver cdylib into a synthetic <root>/driver/
    // tree, mirroring the production library-root layout the scanner
    // expects.
    let root = tempfile::tempdir().expect("tempdir");
    let driver_dir = root.path().join("driver");
    fs::create_dir_all(&driver_dir).expect("create driver dir");
    let dest = driver_dir.join(src.file_name().expect("filename"));
    fs::copy(&src, &dest).expect("copy cdylib");

    let results = LoadedNetGrpc::from_path_scan(root.path());
    assert_eq!(results.len(), 1, "expected exactly one staged entry, got {}", results.len());
    let driver = results
        .into_iter()
        .next()
        .expect("first entry")
        .expect("scan ok");

    // Drop runs vtable.shutdown + vtable.destroy through the FFI
    // boundary; reaching here without panic confirms the scan-path
    // construct + drop lifecycle is sound.
    drop(driver);
}

#[tokio::test]
async fn from_path_scan_on_missing_driver_dir_returns_empty_vec() {
    let root = tempfile::tempdir().expect("tempdir");
    // root/driver/ does not exist — scan layer warns and returns empty.
    let results = LoadedNetGrpc::from_path_scan(root.path());
    assert!(results.is_empty(), "expected empty; got {} entries", results.len());
}

#[tokio::test]
async fn load_from_missing_path_reports_library_open_error() {
    use reovim_subsys_driver_loader::LoadError;
    let missing = PathBuf::from("/definitely/not/a/real/path-net-grpc.so");
    match LoadedNetGrpc::load_from_path(&missing) {
        Err(LoadError::LibraryOpen(_)) => (),
        Ok(_) => panic!("expected LibraryOpen error on missing path"),
        Err(other) => panic!("unexpected error kind: {other:?}"),
    }
}

#[tokio::test]
async fn serve_rejects_non_empty_descriptors_with_sp02b_marker() {
    use reovim_subsys_net::{NetError, ServiceDescriptor};

    let path = cdylib_path("reovim_driver_net_grpc");
    assert!(
        path.exists(),
        "net-grpc cdylib not found at {}; run \
         `cargo build -p reovim-driver-net-grpc` first",
        path.display()
    );

    let driver = LoadedNetGrpc::load_from_path(&path).expect("load net-grpc cdylib");
    // Build a single ServiceDescriptor through the dev-only tower
    // stack. Content is irrelevant; the SP02b guard rejects before any
    // FFI dispatch.
    let svc = tower::util::BoxCloneService::new(Helper);
    let desc = ServiceDescriptor::new("reovim.v3.BufferService", svc);
    let writeback: &'static AtomicU16 = Box::leak(Box::new(AtomicU16::new(0)));

    let result = driver.serve(
        &TransportConfig::tcp_localhost(0),
        vec![desc],
        ShutdownFd(-1),
        ShutdownFd(-1),
        writeback,
    );
    match result {
        Err(NetError::Io(m)) => {
            assert!(m.contains("SP02b"), "expected SP02b marker, got: {m}");
            assert!(m.contains("dispatch_call"), "expected dispatch_call mention, got: {m}");
        }
        other => panic!("expected NetError::Io with SP02b marker, got {other:?}"),
    }
}

/// Trivial tower service used only to satisfy `BoxCloneService::new`'s
/// `Service` bound; never invoked.
#[derive(Clone)]
struct Helper;

impl tower::Service<http::Request<tonic::body::BoxBody>> for Helper {
    type Response = http::Response<tonic::body::BoxBody>;
    type Error = std::convert::Infallible;
    type Future = std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<Self::Response, Self::Error>> + Send + 'static>,
    >;

    fn poll_ready(
        &mut self,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        std::task::Poll::Ready(Ok(()))
    }

    fn call(&mut self, _req: http::Request<tonic::body::BoxBody>) -> Self::Future {
        Box::pin(async {
            use http_body_util::{BodyExt, Empty};
            let empty = Empty::<bytes::Bytes>::new();
            let mapped = BodyExt::map_err(empty, |never| match never {});
            Ok(http::Response::new(tonic::body::BoxBody::new(mapped)))
        })
    }
}
