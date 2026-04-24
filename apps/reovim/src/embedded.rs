//! Embedded composition: server + one client, both in-process.
//!
//! Boots a [`reovim_app_server`]-bootstrapped [`reovim_server::Server`]
//! in one tokio task and the selected client in another, connected by
//! the transport chosen on the launcher CLI (`inproc` by default). The
//! client's exit is the application's exit; the server task is aborted
//! after the client returns.
//!
//! Signal forwarding and graceful-shutdown sequencing (client drains
//! then server stops on a broadcast signal) is the 2b.E deliverable
//! and lands with `lifecycle.rs`. This file today uses
//! [`tokio::task::JoinHandle::abort`] as the server-stop mechanism,
//! which is sufficient for the happy path exercised by the smoke
//! tests.

use std::{io, sync::Arc};

use reovim_server::{Server, ServerConfig, inproc_channel_pair};

use crate::{
    lifecycle::ShutdownCoord,
    subprocess::ClientKind,
    transport::{LaunchMode, TransportChoice, TransportError, TransportKind},
};

/// Resolved arguments for [`run_embedded`].
///
/// Built by `lib::run` from the parsed [`crate::Cli`]; kept as a
/// separate type so the unit tests can construct valid inputs
/// without going through clap.
#[derive(Clone, Debug)]
pub struct EmbeddedArgs {
    /// Which client the launcher boots alongside the server.
    pub client: ClientKind,
    /// Validated transport choice.
    pub transport: TransportChoice,
}

impl EmbeddedArgs {
    /// Resolve CLI flags into embedded arguments.
    ///
    /// `transport_kind` is the raw user-facing flag (`None` defaults
    /// to [`TransportKind::Inproc`]). Address flags are forwarded to
    /// [`TransportChoice::resolve`].
    ///
    /// # Errors
    ///
    /// Propagates [`TransportError`] when the transport validation
    /// rejects the combination.
    pub fn resolve(
        client: ClientKind,
        transport_kind: Option<TransportKind>,
        uds_path: Option<&std::path::Path>,
        tcp_addr: Option<&str>,
    ) -> Result<Self, TransportError> {
        let kind = transport_kind.unwrap_or(TransportKind::Inproc);
        let transport = TransportChoice::resolve(kind, LaunchMode::Embedded, uds_path, tcp_addr)?;
        Ok(Self { client, transport })
    }
}

/// Boot the embedded composition.
///
/// # Errors
///
/// Propagates any I/O error raised by the embedded server task, the
/// embedded client task, or the transport validation.
#[cfg_attr(coverage_nightly, coverage(off))]
pub async fn run_embedded(args: EmbeddedArgs) -> io::Result<()> {
    match args.transport {
        TransportChoice::Inproc => run_inproc(args.client).await,
        TransportChoice::Pipe => Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "embedded mode over OS pipe is not yet wired; use \
             --transport inproc (default) or --transport tcp",
        )),
        TransportChoice::Uds { .. } | TransportChoice::Tcp { .. } => Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "embedded mode over uds/tcp is not yet wired (tracked \
             under #769); use --transport inproc (default)",
        )),
    }
}

/// Inproc path: duplex stream shared between server and chosen client.
#[cfg_attr(coverage_nightly, coverage(off))]
async fn run_inproc(client: ClientKind) -> io::Result<()> {
    let (server_half, client_half) = inproc_channel_pair();
    let server = Arc::new(Server::new(ServerConfig::inproc()));

    // LIFECYCLE: Route ctrl-c through the `ShutdownCoord` broadcast so
    // signal delivery is decoupled from the shutdown target. Today the
    // inproc path has one subscriber (the server listener below);
    // keeping the indirection isolates the select!'s ctrl-c arm from
    // any future fan-out to additional drain targets.
    let coord = ShutdownCoord::new();
    let mut server_rx = coord.subscribe();
    let server_for_signal = Arc::clone(&server);
    let signal_listener = tokio::spawn(async move {
        if server_rx.recv().await.is_ok() {
            let _ = server_for_signal.shutdown().await;
        }
    });

    let result = run_inproc_with(Arc::clone(&server), server_half, async move {
        tokio::select! {
            res = spawn_client(client, client_half) => res,
            sig = tokio::signal::ctrl_c() => {
                let _ = coord.notify_server();
                sig
            },
        }
    })
    .await;

    signal_listener.abort();
    result
}

/// Orchestrate the embedded composition: run `client_fut` concurrently
/// with a server task, then fire [`Server::shutdown`] and join the
/// server task.
///
/// Sequencing invariant:
/// 1. client task awaits,
/// 2. client exits,
/// 3. [`Server::shutdown`] fires on the cloned handle,
/// 4. server drains in-flight RPCs and the tonic loop returns,
/// 5. server task joins,
/// 6. this function returns the client's result.
///
/// Exposed so the `embedded_smoke` integration test can inject a
/// mock client future and observe the ordering without a real
/// tonic-over-duplex wiring.
///
/// # Errors
///
/// Propagates server shutdown errors, server-task join errors, the
/// server's final `run_inproc` result, and the client future's error.
#[cfg_attr(coverage_nightly, coverage(off))]
pub async fn run_inproc_with<F>(
    server: Arc<Server>,
    server_half: tokio::io::DuplexStream,
    client_fut: F,
) -> io::Result<()>
where
    F: std::future::Future<Output = io::Result<()>>,
{
    // LIFECYCLE: spawn the server on a separate task so the client
    // future can own the main task. The `abort_handle` is the escape
    // hatch for a shutdown that tonic cannot drain (see the timeout
    // block below).
    let server_for_stop = Arc::clone(&server);
    let server_task = tokio::spawn(async move { server.run_inproc(server_half).await });
    let server_abort = server_task.abort_handle();

    let client_result = client_fut.await;

    // LIFECYCLE: client has exited — signal graceful server shutdown.
    server_for_stop.shutdown().await?;

    // Tonic drains in-flight RPCs on graceful shutdown. A connection
    // stuck before the HTTP/2 preface (or any non-RPC read) cannot
    // drain, so the serve loop would block indefinitely. Fall back to
    // abort after a short grace window so the embedded smoke tests
    // (and any future idle-client path) converge.
    let server_join = tokio::time::timeout(std::time::Duration::from_secs(2), server_task)
        .await
        .map_or_else(
            |_| {
                server_abort.abort();
                None
            },
            Some,
        );

    // The client's exit is the application's exit (invariant #6). Once
    // the client has returned, any remaining server-side error is
    // diagnostic tail: surface it only when the client itself was Ok.
    client_result?;
    match server_join {
        Some(Ok(server_result)) => server_result,
        Some(Err(join_err)) => Err(io::Error::other(format!("server task: {join_err}"))),
        None => Ok(()),
    }
}

/// Dispatch to the feature-gated client runtime.
#[cfg_attr(coverage_nightly, coverage(off))]
async fn spawn_client(client: ClientKind, stream: tokio::io::DuplexStream) -> io::Result<()> {
    // Silence `unused` on non-feature builds where no arm consumes the
    // stream — the launcher still compiles in that configuration.
    let _ = stream;
    match client {
        ClientKind::Tui => run_tui_client().await,
        ClientKind::Cli => run_cli_client().await,
        ClientKind::Web => run_web_client().await,
    }
}

#[cfg(feature = "embedded-tui")]
#[cfg_attr(coverage_nightly, coverage(off))]
// The server-side tonic-over-DuplexStream loop is live in
// `server/lib/server/src/transport_inproc.rs`; the client side needs
// a `tonic::transport::Channel` built over the `DuplexStream` half
// via `Endpoint::connect_with_connector`, which requires direct deps
// on `tower` + `hyper-util` that the TUI client crate does not yet
// carry. The in-process connect path lands with #769 sub-phase 2b.E
// alongside `ShutdownCoord` and the smoke-test harness. The `async`
// signature is reserved for that wiring — a sync stub would force a
// breaking signature change later.
#[allow(clippy::unused_async)]
async fn run_tui_client() -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "embedded TUI client in-process DuplexStream connect is \
         tracked under #769 sub-phase 2b.E; the server side is live \
         at server/lib/server/src/transport_inproc.rs",
    ))
}

#[cfg(not(feature = "embedded-tui"))]
#[cfg_attr(coverage_nightly, coverage(off))]
#[allow(clippy::unused_async)] // Async signature reserved for 2b.E wiring.
async fn run_tui_client() -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "this build does not include the TUI client runtime; rebuild \
         with --features embedded-tui",
    ))
}

#[cfg(feature = "embedded-cli")]
#[cfg_attr(coverage_nightly, coverage(off))]
#[allow(clippy::unused_async)] // Async signature reserved for 2b.E wiring.
async fn run_cli_client() -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "embedded CLI client in-process DuplexStream connect is \
         tracked under #769 sub-phase 2b.E; the server side is live \
         at server/lib/server/src/transport_inproc.rs",
    ))
}

#[cfg(not(feature = "embedded-cli"))]
#[cfg_attr(coverage_nightly, coverage(off))]
#[allow(clippy::unused_async)] // Async signature reserved for 2b.E wiring.
async fn run_cli_client() -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "this build does not include the CLI client runtime; rebuild \
         with --features embedded-cli",
    ))
}

#[cfg(feature = "embedded-web")]
#[cfg_attr(coverage_nightly, coverage(off))]
#[allow(clippy::unused_async)] // Async signature reserved for 2b.E wiring.
async fn run_web_client() -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "the embedded web runtime is a stub; see #769 for the SSR \
         follow-on",
    ))
}

#[cfg(not(feature = "embedded-web"))]
#[cfg_attr(coverage_nightly, coverage(off))]
#[allow(clippy::unused_async)] // Async signature reserved for 2b.E wiring.
async fn run_web_client() -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "this build does not include the web client runtime; rebuild \
         with --features embedded-web",
    ))
}

#[cfg(test)]
#[path = "embedded_tests.rs"]
mod tests;
