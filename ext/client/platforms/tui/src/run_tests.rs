//! Smoke test for `run(TuiArgs { headless: true, .. })` against a
//! live `reovim-server` via `tools/testing`.
//!
//! `run()` in headless mode blocks on `tokio::signal::ctrl_c()` after
//! the handshake succeeds. The test wraps it in a timeout: a returned
//! `Err` within the window means the connect / handshake / dispatch
//! plumbing is broken, while a timeout elapse means `run()` reached
//! steady state — exactly what the smoke test is asserting.

use std::time::Duration;

use {
    reovim_testing::TestServerHarness,
    super::run::{TuiArgs, run},
};

#[tokio::test]
async fn headless_run_reaches_steady_state_against_live_server() {
    let harness = TestServerHarness::spawn_with_name("platform_tui_run_smoke")
        .await
        .expect("harness spawn");
    let args = TuiArgs {
        grpc: format!("127.0.0.1:{}", harness.port()),
        headless: true,
        width: 20,
        height: 4,
        log: None,
    };

    let outcome = tokio::time::timeout(Duration::from_secs(2), run(args)).await;

    match outcome {
        Err(_elapsed) => {}
        Ok(Ok(())) => panic!("run() returned Ok before timeout; expected to block on shutdown"),
        Ok(Err(err)) => panic!("run() failed to reach steady state: {err}"),
    }
}
