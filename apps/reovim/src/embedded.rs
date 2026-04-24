//! Embedded composition: server + one client, both in-process.
//!
//! Boots a [`reovim_app_server`]-bootstrapped [`reovim_server::Server`]
//! in one tokio task and the selected client in another, connected by
//! the transport chosen on the launcher CLI (`inproc` by default). The
//! client's exit is the application's exit; the server task is drained
//! (or aborted as a fallback) after the client returns.
//!
//! Transport status after #769 2b.G:
//! - `inproc`: server-side tonic-over-DuplexStream loop and client-side
//!   `Endpoint::connect_with_connector` connector are both live. The
//!   launcher hands one duplex half to [`Server::run_inproc`] and the
//!   other to [`reovim_app_tui::run_with_stream`].
//! - `uds`: server-side [`Server::run_unix`] listener bound at
//!   `$XDG_RUNTIME_DIR/reovim-<pid>.sock` (or `--uds-path`); client-side
//!   routes through the `uds://<path>` prefix on the TUI's `grpc` arg.
//! - `tcp`: fully wired via [`Server::run_until`]. The launcher
//!   bootstraps a real gRPC server, waits for the bound port via a
//!   `oneshot::Sender<u16>`, and points the embedded TUI at the
//!   resulting `127.0.0.1:<port>` address.
//! - `pipe`: still `Unsupported` — needs a tonic-over-pipe connector on
//!   the client side (separate follow-on, no server-side listener
//!   exists yet).
//!
//! Signal forwarding and graceful-shutdown sequencing (client drains
//! then server stops on a broadcast signal) land through
//! [`ShutdownCoord`] in `lifecycle.rs`. The 2s abort fallback in
//! [`run_inproc_with`] / [`finalize_server_task`] catches connections
//! stuck before the HTTP/2 preface where tonic's drain cannot converge.

use std::{io, sync::Arc, time::Duration};

use reovim_server::{Server, ServerConfig, inproc_channel_pair};

use crate::{
    lifecycle::ShutdownCoord,
    subprocess::ClientKind,
    transport::{LaunchMode, TransportChoice, TransportError, TransportKind},
};

/// Grace window between `Server::shutdown` and the server task join.
/// Tonic drains in-flight RPCs on graceful shutdown, but a connection
/// stuck before the HTTP/2 preface cannot drain — an abort after this
/// window catches that case so embedded lifecycles always converge.
const SHUTDOWN_DRAIN_TIMEOUT: Duration = Duration::from_secs(2);

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
        TransportChoice::Tcp { addr } => run_tcp(args.client, addr).await,
        TransportChoice::Pipe => Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "embedded mode over OS pipe requires a tonic-over-pipe \
             client connector (separate follow-on). Use \
             --transport inproc (default) or --transport tcp.",
        )),
        #[cfg(unix)]
        TransportChoice::Uds { path } => run_uds(args.client, path).await,
        #[cfg(not(unix))]
        TransportChoice::Uds { .. } => Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "--transport uds is not available on this platform; \
             use --transport tcp",
        )),
    }
}

/// Inproc path: duplex stream shared between server and chosen client.
///
/// LIFECYCLE: bootstraps a production-equivalent server (modules,
/// bridges, domain driver) and runs its tonic-over-DuplexStream loop
/// on a background task. The client future owns the main task; when
/// it exits, `Server::shutdown` drains the serve loop.
#[cfg_attr(coverage_nightly, coverage(off))]
async fn run_inproc(client: ClientKind) -> io::Result<()> {
    let (server_half, client_half) = inproc_channel_pair();
    let server = Arc::new(build_inproc_server());

    // Broadcast: today the inproc path has one subscriber (the shutdown
    // listener); the indirection keeps ctrl-c decoupled from the
    // shutdown target.
    let coord = ShutdownCoord::new();
    let mut server_rx = coord.subscribe();
    let server_for_signal = Arc::clone(&server);
    let signal_listener = tokio::spawn(async move {
        if server_rx.recv().await.is_ok() {
            let _ = server_for_signal.shutdown().await;
        }
    });

    let server_for_task = Arc::clone(&server);
    let server_task = tokio::spawn(async move { server_for_task.run_inproc(server_half).await });
    let server_abort = server_task.abort_handle();

    let client_result = tokio::select! {
        res = spawn_client(client, client_half) => res,
        sig = tokio::signal::ctrl_c() => {
            let _ = coord.notify_server();
            sig
        },
    };

    // Client exited → drain the server (idempotent; the signal listener
    // may have fired the same shutdown already).
    server.shutdown().await?;
    let server_result = finalize_server_task(server_task, server_abort).await;
    signal_listener.abort();

    client_result?;
    server_result
}

/// Build a fully-bootstrapped `Server` configured for in-process
/// transport.
///
/// Mirrors [`build_tcp_server`]: bootstraps modules, registers
/// bridges, attaches the module service + optional domain driver.
/// Required so the inproc client receives the same service surface
/// (modules, bridges, domain driver) as the TCP composition — without
/// it, the duplex-stream gRPC handshake would succeed but subsequent
/// RPCs would hit empty registries.
#[cfg_attr(coverage_nightly, coverage(off))]
fn build_inproc_server()
-> reovim_server::Server<reovim_app_server::module_service::RunnerGrpcModuleService> {
    let bootstrap = reovim_app_server::bootstrap::bootstrap_runtime();
    let runner_module_service = reovim_app_server::module_service::RunnerGrpcModuleService::new(
        Arc::clone(&bootstrap.module_registry),
        Arc::clone(&bootstrap.module_ctx),
    );
    let mut server = Server::new(ServerConfig::inproc())
        .with_initial_session_state(bootstrap.session_state)
        .with_bridges(bootstrap.bridges)
        .with_module_service(runner_module_service);
    if let Some(driver) = bootstrap.domain_driver {
        server = server.with_domain_driver(driver);
    }
    server
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

    // Same 2s abort fallback as [`finalize_server_task`]: tonic cannot
    // drain a connection stuck before the HTTP/2 preface, so the embedded
    // smoke tests (and any future idle-client path) need an explicit
    // converge window. See [`SHUTDOWN_DRAIN_TIMEOUT`] for the shared knob.
    let server_join = tokio::time::timeout(SHUTDOWN_DRAIN_TIMEOUT, server_task)
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

/// UDS path: the launcher binds a `reovim_server` UDS listener at
/// `path` and points the embedded client at `"uds://<path>"`.
///
/// LIFECYCLE: mirrors `run_tcp`. The server runs on a background task;
/// the client future owns exit. When the client returns,
/// [`Server::shutdown`] drains tonic's in-flight RPCs with a 2s abort
/// fallback via [`finalize_server_task`] for connections stuck before
/// the HTTP/2 preface.
#[cfg(unix)]
#[cfg_attr(coverage_nightly, coverage(off))]
async fn run_uds(client: ClientKind, path: std::path::PathBuf) -> io::Result<()> {
    let server = Arc::new(build_uds_server(path.clone()));
    let shutdown = ShutdownCoord::new();
    let mut shutdown_rx = shutdown.subscribe();
    let server_for_signal = Arc::clone(&server);
    let signal_task = tokio::spawn(async move {
        if shutdown_rx.recv().await.is_ok() {
            let _ = server_for_signal.shutdown().await;
        }
    });

    let server_for_task = Arc::clone(&server);
    let path_for_task = path.clone();
    let server_task = tokio::spawn(async move { server_for_task.run_unix(&path_for_task).await });
    let server_abort = server_task.abort_handle();

    let connect_addr = format!("uds://{}", path.display());
    let client_result = tokio::select! {
        res = spawn_client_tcp(client, connect_addr) => res,
        sig = tokio::signal::ctrl_c() => {
            let _ = shutdown.notify_server();
            sig
        },
    };

    // Drain then join. The second notify is benign ("no listeners").
    let _ = shutdown.notify_server();
    server.shutdown().await?;
    let join_result = finalize_server_task(server_task, server_abort).await;
    signal_task.abort();
    // Best-effort socket-file cleanup (the listener-drop already unlinks
    // on recent tokio; this is a paranoia step for EEXIST on relaunch).
    let _ = std::fs::remove_file(&path);
    client_result?;
    join_result
}

/// Build a fully-bootstrapped `Server<RunnerGrpcModuleService>`
/// configured for a UDS listener at `path`.
///
/// Mirrors [`build_tcp_server`]; the only difference is the transport
/// discriminant.
#[cfg(unix)]
#[cfg_attr(coverage_nightly, coverage(off))]
fn build_uds_server(
    path: std::path::PathBuf,
) -> Server<reovim_app_server::module_service::RunnerGrpcModuleService> {
    let bootstrap = reovim_app_server::bootstrap::bootstrap_runtime();
    let runner_module_service = reovim_app_server::module_service::RunnerGrpcModuleService::new(
        Arc::clone(&bootstrap.module_registry),
        Arc::clone(&bootstrap.module_ctx),
    );
    let mut server = Server::new(ServerConfig::unix_socket(path))
        .with_initial_session_state(bootstrap.session_state)
        .with_bridges(bootstrap.bridges)
        .with_module_service(runner_module_service);
    if let Some(driver) = bootstrap.domain_driver {
        server = server.with_domain_driver(driver);
    }
    server
}

/// TCP path: the launcher spawns a real gRPC server on `addr.port()`
/// (0 = OS-assigned) and points the embedded client at the bound
/// `127.0.0.1:<port>` address. Server and client share one process;
/// the transport is a loopback TCP socket.
///
/// LIFECYCLE: mirrors `run_inproc` — server runs on a background
/// tokio task, the client future on the main task owns the exit.
/// When the client returns, [`Server::shutdown`] fires to drain
/// tonic's in-flight RPCs, with a 2s abort fallback for connections
/// stuck before the HTTP/2 preface.
#[cfg_attr(coverage_nightly, coverage(off))]
async fn run_tcp(client: ClientKind, addr: std::net::SocketAddr) -> io::Result<()> {
    let server = build_tcp_server(addr.port());
    let (port_tx, port_rx) = tokio::sync::oneshot::channel::<u16>();
    let shutdown = ShutdownCoord::new();
    let mut shutdown_rx = shutdown.subscribe();

    let server_task = tokio::spawn(async move {
        server
            .run_until(
                async move {
                    // Await the broadcast; a recv error means every
                    // sender was dropped, treated as "please stop".
                    let _ = shutdown_rx.recv().await;
                },
                Some(port_tx),
            )
            .await
    });
    let server_abort = server_task.abort_handle();

    // LIFECYCLE: wait for the port to be reported before connecting.
    let bound_port = port_rx
        .await
        .map_err(|_| io::Error::other("embedded server exited before port was reported"))?;
    let connect_addr = format!("127.0.0.1:{bound_port}");

    let client_result = tokio::select! {
        res = spawn_client_tcp(client, connect_addr) => res,
        sig = tokio::signal::ctrl_c() => {
            // Wake the server's drain path; idempotent across the
            // two fires (ctrl_c here and the post-select fire below).
            let _ = shutdown.notify_server();
            sig
        },
    };

    // LIFECYCLE: client exited → drain the server. The second
    // `notify_server` after the ctrl_c branch already fired is a
    // benign "no listeners" error we intentionally ignore.
    let _ = shutdown.notify_server();
    finalize_server_task(server_task, server_abort).await?;
    client_result
}

/// Build a fully-bootstrapped `Server` configured for gRPC on `port`.
///
/// LIFECYCLE: duplicates the composition-root wiring from
/// `reovim_app_server::run_server` (bootstrap modules, register
/// bridges, attach module service + optional domain driver) so the
/// embedded launcher serves a functionally-identical gRPC surface
/// without shelling out to the standalone bin.
#[cfg_attr(coverage_nightly, coverage(off))]
fn build_tcp_server(
    port: u16,
) -> Arc<Server<reovim_app_server::module_service::RunnerGrpcModuleService>> {
    let bootstrap = reovim_app_server::bootstrap::bootstrap_runtime();
    let runner_module_service = reovim_app_server::module_service::RunnerGrpcModuleService::new(
        Arc::clone(&bootstrap.module_registry),
        Arc::clone(&bootstrap.module_ctx),
    );
    let mut server = Server::new(ServerConfig::grpc(port))
        .with_initial_session_state(bootstrap.session_state)
        .with_bridges(bootstrap.bridges)
        .with_module_service(runner_module_service);
    if let Some(driver) = bootstrap.domain_driver {
        server = server.with_domain_driver(driver);
    }
    Arc::new(server)
}

/// Await the server task with a 2s grace after `Server::shutdown`;
/// abort and return Ok if tonic cannot drain in time.
#[cfg_attr(coverage_nightly, coverage(off))]
async fn finalize_server_task(
    server_task: tokio::task::JoinHandle<io::Result<()>>,
    server_abort: tokio::task::AbortHandle,
) -> io::Result<()> {
    match tokio::time::timeout(SHUTDOWN_DRAIN_TIMEOUT, server_task).await {
        Ok(Ok(result)) => result,
        Ok(Err(join_err)) => Err(io::Error::other(format!("server task: {join_err}"))),
        Err(_) => {
            server_abort.abort();
            Ok(())
        }
    }
}

/// Dispatch to the feature-gated client runtime over a TCP connect
/// string (`host:port`). Parallels [`spawn_client`] for the inproc path.
#[cfg_attr(coverage_nightly, coverage(off))]
async fn spawn_client_tcp(client: ClientKind, connect_addr: String) -> io::Result<()> {
    match client {
        ClientKind::Tui => run_tui_client_tcp(connect_addr).await,
        ClientKind::Cli => run_cli_client_tcp(connect_addr).await,
        ClientKind::Web => run_web_client_tcp(connect_addr).await,
    }
}

/// Dispatch to the feature-gated client runtime over an in-process
/// `DuplexStream`.
#[cfg_attr(coverage_nightly, coverage(off))]
async fn spawn_client(client: ClientKind, stream: tokio::io::DuplexStream) -> io::Result<()> {
    match client {
        ClientKind::Tui => run_tui_client(stream).await,
        ClientKind::Cli => {
            drop(stream);
            run_cli_client().await
        }
        ClientKind::Web => {
            drop(stream);
            run_web_client().await
        }
    }
}

/// Inproc TUI client: hand the client-side `DuplexStream` to the
/// platform's [`reovim_app_tui::run_with_stream`] entry.
#[cfg(feature = "embedded-tui")]
#[cfg_attr(coverage_nightly, coverage(off))]
async fn run_tui_client(stream: tokio::io::DuplexStream) -> io::Result<()> {
    reovim_app_tui::run_with_stream(stream, reovim_app_tui::TuiArgs::default()).await
}

#[cfg(not(feature = "embedded-tui"))]
#[cfg_attr(coverage_nightly, coverage(off))]
#[allow(clippy::unused_async, clippy::needless_pass_by_value)]
async fn run_tui_client(_stream: tokio::io::DuplexStream) -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "this build does not include the TUI client runtime; rebuild \
         with --features embedded-tui",
    ))
}

/// TCP path TUI client: delegates to
/// [`reovim_app_tui::run`] with a loopback-TCP connect string.
///
/// The launcher runs interactive mode by default (the user-facing
/// `reovim` binary owns the terminal); headless and width/height
/// defaults mirror the standalone bin's clap defaults.
#[cfg(feature = "embedded-tui")]
#[cfg_attr(coverage_nightly, coverage(off))]
async fn run_tui_client_tcp(connect_addr: String) -> io::Result<()> {
    let args = reovim_app_tui::TuiArgs {
        grpc: connect_addr,
        headless: false,
        width: 120,
        height: 40,
        log: None,
    };
    reovim_app_tui::run(args).await
}

#[cfg(not(feature = "embedded-tui"))]
#[cfg_attr(coverage_nightly, coverage(off))]
#[allow(clippy::unused_async, clippy::needless_pass_by_value)]
async fn run_tui_client_tcp(_connect_addr: String) -> io::Result<()> {
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

/// TCP path CLI client: returns `Unsupported` with a pointer at the
/// standalone `reovim-cli` bin for one-shot commands.
///
/// The CLI is a one-shot request/response tool (its clap schema
/// requires a subcommand). The embedded-launcher's "boot server +
/// client" flow does not compose with that lifecycle, and the user's
/// subcommand cannot be plumbed through the launcher's CLI surface
/// without inventing a `reovim -- cli ...` pass-through. Until that
/// surface is designed, the embedded CLI slot is a diagnostic pointer.
#[cfg(feature = "embedded-cli")]
#[cfg_attr(coverage_nightly, coverage(off))]
#[allow(clippy::unused_async, clippy::needless_pass_by_value)]
async fn run_cli_client_tcp(_connect_addr: String) -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "embedded CLI client over TCP is not yet wired — the CLI is \
         one-shot and needs a pass-through subcommand shape; tracked \
         under #769 follow-on. Use `reovim cli --grpc HOST:PORT …` \
         against a separately-running server for today.",
    ))
}

#[cfg(not(feature = "embedded-cli"))]
#[cfg_attr(coverage_nightly, coverage(off))]
#[allow(clippy::unused_async, clippy::needless_pass_by_value)]
async fn run_cli_client_tcp(_connect_addr: String) -> io::Result<()> {
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

/// TCP path web client: today the web runtime is a scaffold with no
/// gRPC connect surface. Returns `Unsupported` pending the SSR
/// follow-on.
#[cfg(feature = "embedded-web")]
#[cfg_attr(coverage_nightly, coverage(off))]
#[allow(clippy::unused_async, clippy::needless_pass_by_value)]
async fn run_web_client_tcp(_connect_addr: String) -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "the embedded web runtime is a stub; see #769 for the SSR \
         follow-on",
    ))
}

#[cfg(not(feature = "embedded-web"))]
#[cfg_attr(coverage_nightly, coverage(off))]
#[allow(clippy::unused_async, clippy::needless_pass_by_value)]
async fn run_web_client_tcp(_connect_addr: String) -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "this build does not include the web client runtime; rebuild \
         with --features embedded-web",
    ))
}

#[cfg(test)]
#[path = "embedded_tests.rs"]
mod tests;
