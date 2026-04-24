#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! `reovim` launcher library.
//!
//! Dispatches the top-level CLI to one of four paths:
//!
//! 1. Passthrough (`reovim server|tui|cli|module|web …`) — spawn the
//!    matching sibling bin via [`subprocess::run`] exactly as the 2a
//!    launcher did.
//! 2. External gRPC (`--external-grpc HOST:PORT`) — the launcher spawns
//!    only the chosen client and points it at an already-running
//!    server. Subprocess wiring is tracked as the 2b.E deliverable.
//! 3. Subprocess (`--subprocess`) — launcher forks server + client as
//!    separate sibling-bin processes. Also 2b.E territory.
//! 4. Embedded (default) — launcher runs server + client in a single
//!    process connected by the selected in-process transport (`inproc`
//!    by default). See [`embedded::run_embedded`].

pub mod embedded;
pub mod subprocess;
pub mod transport;

pub use subprocess::{Cli, ClientKind, Cmd};

/// Top-level entry point.
///
/// Parses-already `Cli` and dispatches to the appropriate launcher
/// path (embedded, subprocess, external-gRPC, or passthrough).
///
/// Embedded mode boots the reovim server and the chosen client in
/// one process, connected by the selected in-process transport
/// (default: `inproc`, a `tokio::io::DuplexStream`). The
/// server-side tonic-over-duplex loop is live; the client-side
/// `DuplexStream` connect path (via
/// `tonic::transport::Endpoint::connect_with_connector`) lands with
/// `#769` sub-phase 2b.E alongside `ShutdownCoord` and the
/// integration smoke tests.
///
/// # Errors
///
/// Propagates any `std::io::Error` raised by the dispatched path.
#[cfg_attr(coverage_nightly, coverage(off))]
pub fn run(cli: Cli) -> std::io::Result<()> {
    // Passthrough subcommands bypass the embedded-mode flags entirely.
    if cli.command.is_some() {
        return subprocess::run(cli);
    }

    if cli.subprocess || cli.external_grpc.is_some() || cli.no_server {
        return Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "subprocess composition (--subprocess / --external-grpc / \
             --no-server) is not yet wired; tracked under #769",
        ));
    }

    let args = embedded::EmbeddedArgs::resolve(
        cli.client,
        cli.transport,
        cli.uds_path.as_deref(),
        cli.tcp_addr.as_deref(),
    )
    .map_err(std::io::Error::from)?;

    run_embedded_on_tokio_runtime(args)
}

/// Spin up a tokio runtime and run the embedded composition on it.
///
/// Separated from [`run`] so `main.rs` can stay a pure sync
/// `Cli::parse()` + `run()` shell.
#[cfg_attr(coverage_nightly, coverage(off))]
fn run_embedded_on_tokio_runtime(args: embedded::EmbeddedArgs) -> std::io::Result<()> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    runtime.block_on(embedded::run_embedded(args))
}
