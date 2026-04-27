//! Integration smoke test for subprocess-mode SIGINT propagation.
//!
//! Placeholder scaffold: reliably ordering `cargo run --bin reovim
//! --subprocess --transport uds`, a SIGINT from the test driver, and
//! tokio runtime teardown inside the launcher is flaky in the CI
//! matrix today. The headless smoke (server + TUI over UDS, assert
//! both children exit with SIGINT code within 6 s) lands with the
//! `/e2e` harness pass.
//!
//! TODO(#769): un-`ignore` once the `/e2e` driver can spawn the
//! launcher + capture `ExitStatus::signal() == Some(libc::SIGINT)`
//! for the children deterministically.

#![cfg(unix)]
#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]

#[ignore = "subprocess SIGINT smoke lands with /e2e harness — TODO(#769)"]
#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn sigint_propagates_to_server_and_client() {
    // Intentional scaffold. The live check flow:
    //   1. Spawn `cargo run -p reovim-app-launcher -- --subprocess --transport uds`.
    //   2. Wait for both children to advertise readiness (TUI draws a
    //      frame; server accepts a connection).
    //   3. `libc::kill(launcher_pid, libc::SIGINT)`.
    //   4. `launcher.wait()` returns within 6 s.
    //   5. Per-child exit status (captured via the harness' process
    //      table) matches `ExitStatus::signal() == Some(libc::SIGINT)`
    //      or a 128+SIGINT exit code for the client.
}
