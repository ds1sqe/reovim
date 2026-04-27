//! Transport matrix parameterized over the 7 valid (launch-mode,
//! transport) pairs and the 2 invalid-pair categories.
//!
//! ## Valid matrix (7 pairs)
//!
//! | Launch mode  | Transport | Location in this file               |
//! |--------------|-----------|-------------------------------------|
//! | Embedded     | Inproc    | `embedded_inproc_roundtrip`         |
//! | Embedded     | Tcp       | `embedded_tcp_roundtrip`            |
//! | Embedded     | Uds       | `embedded_uds_roundtrip` (unix)     |
//! | Embedded     | Pipe      | `embedded_pipe_roundtrip`           |
//! | Subprocess   | Tcp       | `subprocess_tcp_roundtrip`          |
//! | Subprocess   | Uds       | `subprocess_uds_roundtrip` (unix)   |
//! | `ExternalGrpc` | Tcp       | `external_grpc_tcp_roundtrip`      |
//!
//! All keystroke→frame roundtrip tests in this file are `#[ignore]`d
//! and deferred to the `/e2e` harness (TODO(#769)). As of 2b.G the
//! client-side tonic-over-`DuplexStream` connector (Inproc) and the
//! server-side `Server::run_unix` listener (Uds) are live; the
//! remaining blocker is a full gRPC + TUI lifecycle driver that
//! libtest cannot host deterministically (keystroke injection,
//! frame capture, terminal allocation). The sibling
//! `apps/reovim/tests/embedded_smoke.rs` asserts the embedded
//! sequencing invariant with a mock client future for the Inproc
//! arm; this file records the deferral shape and the invalid-pair
//! rejections that *do* run in-process.
//!
//! ## Invalid-pair matrix
//!
//! The two categories asserted here, in-process, via
//! `TransportChoice::resolve`:
//!
//! 1. `Inproc` under `Subprocess` / `ExternalGrpc` →
//!    `TransportError::InprocRequiresEmbedded`.
//! 2. `{Pipe, Uds}` under `ExternalGrpc` →
//!    `TransportError::ExternalGrpcRequiresTcp`.
//!
//! These are the semantic validation rules layered on top of clap's
//! raw flag parse — the CLI admits all four transport kinds under
//! every launch mode at the parse stage, and the composition logic
//! rejects the incompatible combinations when `EmbeddedArgs::resolve`
//! / `run_subprocess_on_tokio_runtime` lift the raw flags into a
//! [`TransportChoice`].

#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]

use reovim_app_launcher::transport::{LaunchMode, TransportChoice, TransportError, TransportKind};

// --------------------------------------------------------------------
// Invalid-pair matrix (runs in-process, no spawning).
// --------------------------------------------------------------------

#[test]
fn inproc_subprocess_is_rejected() {
    let err = TransportChoice::resolve(TransportKind::Inproc, LaunchMode::Subprocess, None, None)
        .expect_err("inproc + subprocess must be rejected");
    assert_eq!(err, TransportError::InprocRequiresEmbedded);
}

#[test]
fn inproc_external_grpc_is_rejected() {
    let err = TransportChoice::resolve(TransportKind::Inproc, LaunchMode::ExternalGrpc, None, None)
        .expect_err("inproc + external-grpc must be rejected");
    assert_eq!(err, TransportError::InprocRequiresEmbedded);
}

#[test]
fn pipe_external_grpc_is_rejected() {
    let err = TransportChoice::resolve(TransportKind::Pipe, LaunchMode::ExternalGrpc, None, None)
        .expect_err("pipe + external-grpc must be rejected");
    assert_eq!(err, TransportError::ExternalGrpcRequiresTcp);
}

#[test]
fn uds_external_grpc_is_rejected() {
    let err = TransportChoice::resolve(TransportKind::Uds, LaunchMode::ExternalGrpc, None, None)
        .expect_err("uds + external-grpc must be rejected");
    assert_eq!(err, TransportError::ExternalGrpcRequiresTcp);
}

// --------------------------------------------------------------------
// Valid-pair matrix (deferred to /e2e — TODO(#769)).
//
// Each test below spells out the keystroke→frame roundtrip shape the
// /e2e harness will exercise against the launcher binary. The flow
// matches `apps/reovim/tests/subprocess_smoke.rs`: ignore by default,
// leave the protocol as a #[test] body so a future harness pass can
// un-`#[ignore]` without rewriting the scaffolding.
// --------------------------------------------------------------------

#[ignore = "embedded-inproc keystroke roundtrip deferred to /e2e harness — TODO(#769)"]
#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn embedded_inproc_roundtrip() {
    // Flow captured for the /e2e harness:
    //   1. Spawn `reovim --transport inproc` (default).
    //   2. Send `ihello<Esc>` via a headless TUI driver.
    //   3. Capture the rendered frame; assert it contains "hello".
    //   4. Send `:q<CR>`; assert the launcher exits 0.
    //
    // Wiring is live as of 2b.G: `embedded.rs::run_inproc` hands one
    // duplex half to `Server::run_inproc` and the other to
    // `reovim_app_tui::run_with_stream`. The remaining blocker is
    // libtest's inability to host a real TUI event loop
    // deterministically — the /e2e harness owns keystroke injection
    // and frame capture.
}

#[ignore = "embedded-tcp keystroke roundtrip deferred to /e2e harness — TODO(#769)"]
#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn embedded_tcp_roundtrip() {
    // Flow:
    //   1. Spawn `reovim --transport tcp --tcp-addr 127.0.0.1:0`.
    //   2. The launcher boots a real gRPC server on an OS-assigned
    //      port (via `Server::run_until`), extracts the port through
    //      the oneshot, and connects the TUI at `127.0.0.1:<port>`.
    //   3. Send `ihello<Esc>` via the harness' headless TUI driver.
    //   4. Capture frame; assert it contains "hello".
    //   5. Send `:q<CR>`; assert clean exit.
    //
    // The wiring is live as of 2b.G (`embedded::run_tcp`); the blocker
    // here is libtest's inability to host a real TUI event loop
    // deterministically alongside a gRPC stack. The /e2e harness owns
    // that orchestration.
}

#[cfg(unix)]
#[ignore = "embedded-uds keystroke roundtrip deferred to /e2e harness — TODO(#769)"]
#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn embedded_uds_roundtrip() {
    // Flow:
    //   1. Spawn `reovim --transport uds --uds-path /tmp/reovim-matrix.sock`.
    //   2. Same keystroke roundtrip as embedded-tcp.
    //
    // Wiring is live as of 2b.G: `Server::run_unix` is a real tonic-
    // over-UnixListener loop, and the TUI's `--grpc` flag now accepts
    // a `uds://<path>` prefix. Same harness blocker as the other
    // roundtrip tests.
}

#[ignore = "embedded-pipe keystroke roundtrip needs tonic-over-pipe client connector — TODO(#769)"]
#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn embedded_pipe_roundtrip() {
    // Flow:
    //   1. Spawn `reovim --transport pipe`.
    //   2. Launcher wires server + client over an `AsyncRead` +
    //      `AsyncWrite` pair (launcher-internal `tokio::io::pipe` or
    //      `DuplexStream`).
    //   3. Same keystroke roundtrip as embedded-inproc.
    //
    // Blocker: same connector-side gap as embedded-inproc. The
    // server-side `Server::run_pipe` is live; the client side is not.
}

#[ignore = "subprocess-tcp smoke lives in /e2e harness alongside subprocess_smoke.rs — TODO(#769)"]
#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn subprocess_tcp_roundtrip() {
    // Flow:
    //   1. Spawn `reovim --subprocess --transport tcp --tcp-addr 127.0.0.1:0`.
    //   2. The launcher forks `reovim-server --grpc <port>` and
    //      `reovim-tui --grpc 127.0.0.1:<port>`.
    //   3. Send `ihello<Esc>` via the harness' headless TUI driver.
    //   4. Capture frame; assert it contains "hello".
    //   5. SIGINT the launcher; assert both children exit within 6 s.
    //
    // Mirrors `tests/subprocess_smoke.rs`; same flakiness reason for
    // the `#[ignore]`.
}

#[cfg(unix)]
#[ignore = "subprocess-uds smoke lives in /e2e harness alongside subprocess_smoke.rs — TODO(#769)"]
#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn subprocess_uds_roundtrip() {
    // Same shape as subprocess_tcp_roundtrip but with `--transport uds`.
    // The server-side UDS listener must land first (see
    // `embedded_uds_roundtrip` blocker note).
}

#[ignore = "external-grpc smoke lives in /e2e harness — TODO(#769)"]
#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn external_grpc_tcp_roundtrip() {
    // Flow:
    //   1. Start a standalone `reovim-server --grpc 13000` out-of-band.
    //   2. Spawn `reovim --external-grpc 127.0.0.1:13000`.
    //   3. Launcher forks only the client (no server spawn) and wires
    //      it at the external address.
    //   4. Same keystroke roundtrip as subprocess-tcp.
}
