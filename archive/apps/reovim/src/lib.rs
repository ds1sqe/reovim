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
//!    server. See [`subprocess_compose::run_subprocess`].
//! 3. Subprocess (`--subprocess`) — launcher forks server + client as
//!    separate sibling-bin processes. See
//!    [`subprocess_compose::run_subprocess`].
//! 4. Embedded (default) — launcher runs server + client in a single
//!    process connected by the selected in-process transport (`inproc`
//!    by default). See [`embedded::run_embedded`].

pub mod embedded;
pub mod lifecycle;
pub mod subprocess;
pub mod subprocess_compose;
pub mod transport;

pub use subprocess::{Cli, ClientKind, Cmd};

/// Top-level entry point.
///
/// Parses-already `Cli` and dispatches to the appropriate launcher
/// path (embedded, subprocess, external-gRPC, or passthrough).
///
/// Embedded mode boots the reovim server and the chosen client in
/// one process, connected by the selected in-process transport
/// (default: `inproc`, a `tokio::io::DuplexStream`). Both halves of
/// the inproc transport are live — server side via
/// `reovim_server::Server::run_inproc` + `transport_inproc::run`,
/// client side via `reovim_app_tui::run_with_stream`
/// (`Endpoint::connect_with_connector` + `TokioIo`). UDS and TCP
/// transports route through real OS sockets.
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

    // Subprocess, external-gRPC, and --no-server all follow the
    // subprocess-composition path. `--external-grpc` and `--no-server`
    // imply "launcher does not spawn a server"; `--subprocess` alone
    // spawns both server and client.
    if cli.subprocess || cli.external_grpc.is_some() || cli.no_server {
        return run_subprocess_on_tokio_runtime(cli);
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

/// Spin up a tokio runtime and run the subprocess composition on it.
///
/// Resolves the launch mode (Subprocess vs `ExternalGrpc`) and the
/// transport choice from `cli`, then delegates to
/// [`subprocess_compose::run_subprocess`]. The client's exit code is
/// propagated via [`std::process::exit`].
#[cfg_attr(coverage_nightly, coverage(off))]
fn run_subprocess_on_tokio_runtime(cli: Cli) -> std::io::Result<()> {
    let launch_mode = if cli.external_grpc.is_some() {
        transport::LaunchMode::ExternalGrpc
    } else {
        transport::LaunchMode::Subprocess
    };

    // Default transport: external-grpc → tcp; subprocess → uds on unix,
    // tcp elsewhere. Overridable via `--transport`.
    let default_kind = match launch_mode {
        transport::LaunchMode::ExternalGrpc => transport::TransportKind::Tcp,
        transport::LaunchMode::Subprocess => {
            if cfg!(unix) {
                transport::TransportKind::Uds
            } else {
                transport::TransportKind::Tcp
            }
        }
        transport::LaunchMode::Embedded => transport::TransportKind::Inproc,
    };

    let kind = cli.transport.unwrap_or(default_kind);
    let tcp_addr = cli.external_grpc.as_deref().or(cli.tcp_addr.as_deref());

    let transport =
        transport::TransportChoice::resolve(kind, launch_mode, cli.uds_path.as_deref(), tcp_addr)
            .map_err(std::io::Error::from)?;

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    let exit_code = runtime.block_on(subprocess_compose::run_subprocess(
        launch_mode,
        transport,
        cli.client,
        cli.external_grpc,
    ))?;
    std::process::exit(exit_code);
}
