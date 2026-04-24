//! End-to-end integration test for the `reovim cli debug` verbs
//! (#770 Phase 2).
//!
//! Spawns a real `reovim-server` (via the in-crate harness in
//! `tests/common.rs`) with the Phase 0 `reovim-driver-debug-poc`
//! cdylib registered, then spawns the real `reovim-cli` binary
//! (resolved via `reovim_testing::bin_paths::which_reovim_cli`) and
//! asserts that each of `probe`, `observe`, and `drive` produces the
//! expected stdout.
//!
//! This is the first CLI E2E test in the crate; it's load-bearing
//! for the Phase 2 master-plan acceptance ("3 CLI verbs work end-to-
//! end; smoke test runs clean").

#![allow(clippy::result_large_err)]
#![allow(clippy::significant_drop_tightening)]

mod common;

use {
    common::{TestServer, register_poc},
    reovim_server::ClientDebugRegistry,
    reovim_testing::which_reovim_cli,
    std::{sync::Arc, time::Duration},
    tokio::process::Command,
};

/// Give the server a brief settle window after port 0 reports back —
/// the listener binds before the reported port is returned, but the
/// `accept()` loop may not have been polled yet.
async fn settle() {
    tokio::time::sleep(Duration::from_millis(100)).await;
}

async fn start_server_with_poc() -> TestServer {
    let reg = Arc::new(ClientDebugRegistry::new());
    register_poc(&reg, "reovim_driver_debug_poc", "debug-poc");
    let server = TestServer::start(reg).await;
    settle().await;
    server
}

async fn run_cli_await(addr: &str, args: &[&str]) -> std::process::Output {
    let bin = which_reovim_cli();
    assert!(
        bin.exists(),
        "reovim-cli binary not found at {}; run `cargo build -p reovim-app-cli` first",
        bin.display()
    );
    let mut cmd = Command::new(&bin);
    cmd.arg("--grpc").arg(addr);
    cmd.args(args);
    cmd.output().await.expect("cli subprocess")
}

#[tokio::test]
async fn e2e_debug_probe_prints_driver_metadata() {
    let server = start_server_with_poc().await;
    let out =
        run_cli_await(&server.grpc_addr(), &["debug", "probe", "--driver", "debug-poc"]).await;
    assert!(
        out.status.success(),
        "cli exit {:?}; stderr:\n{}",
        out.status,
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("driver: debug-poc"), "stdout:\n{stdout}");
    assert!(stdout.contains("poc-frames"), "stdout:\n{stdout}");
    assert!(stdout.contains("poc-echo"), "stdout:\n{stdout}");
}

#[tokio::test]
async fn e2e_debug_observe_prints_three_frames() {
    let server = start_server_with_poc().await;
    let out = run_cli_await(
        &server.grpc_addr(),
        &[
            "debug",
            "observe",
            "--driver",
            "debug-poc",
            "--schema",
            "poc-frames",
        ],
    )
    .await;
    assert!(
        out.status.success(),
        "cli exit {:?}; stderr:\n{}",
        out.status,
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    // PoC driver yields "frame-2", "frame-1", "frame-0" in that order.
    for frame in ["frame-2", "frame-1", "frame-0"] {
        let mut hex = String::with_capacity(frame.len() * 2);
        for b in frame.bytes() {
            use std::fmt::Write;
            write!(&mut hex, "{b:02x}").unwrap();
        }
        assert!(stdout.contains(&hex), "expected hex of {frame} in stdout:\n{stdout}");
    }
}

#[tokio::test]
async fn e2e_debug_drive_echoes_input() {
    let server = start_server_with_poc().await;
    let out = run_cli_await(
        &server.grpc_addr(),
        &[
            "debug",
            "drive",
            "--driver",
            "debug-poc",
            "--schema",
            "poc-echo",
            "--input",
            "hello",
        ],
    )
    .await;
    assert!(
        out.status.success(),
        "cli exit {:?}; stderr:\n{}",
        out.status,
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    // "hello" = 68656c6c6f
    assert!(stdout.contains("response=68656c6c6f"), "stdout:\n{stdout}");
}

#[tokio::test]
async fn e2e_debug_drive_at_path_file_not_found_errors() {
    let server = start_server_with_poc().await;
    let out = run_cli_await(
        &server.grpc_addr(),
        &[
            "debug",
            "drive",
            "--driver",
            "debug-poc",
            "--schema",
            "poc-echo",
            "--input",
            "@/nonexistent/deadbeef.bin",
        ],
    )
    .await;
    assert!(!out.status.success(), "expected non-zero exit");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("nonexistent/deadbeef.bin"), "stderr:\n{stderr}");
}

#[tokio::test]
async fn e2e_debug_probe_unknown_driver_errors() {
    let server = start_server_with_poc().await;
    let out =
        run_cli_await(&server.grpc_addr(), &["debug", "probe", "--driver", "not-registered"]).await;
    assert!(!out.status.success(), "expected non-zero exit");
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        combined.contains("unknown driver") || combined.contains("not-registered"),
        "expected error message; combined output:\n{combined}"
    );
}
