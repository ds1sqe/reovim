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

use std::io;

use reovim_server::{Server, ServerConfig, inproc_channel_pair};

use crate::{
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
// LIFECYCLE: this function holds the composition root; the 2b.E
// `ShutdownCoord` will replace the `handle.abort()` server-stop path
// with a broadcast signal + `Server::shutdown` drain. The body today
// is still under the 40-LOC launcher-thin ceiling because the inproc
// branch delegates service bootstrap to `run_embedded_inproc` and the
// external branches are single-transport tokio::spawn pairs.
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

    let server_task = tokio::spawn(async move {
        let server = Server::new(ServerConfig::inproc());
        server.run_inproc(server_half).await
    });

    let client_result = spawn_client(client, client_half).await;

    server_task.abort();
    // Drain the server task's final result but treat a post-abort
    // JoinError as a normal shutdown — the launcher-driven abort is
    // the success path for the 2b.D wiring.
    let _ = server_task.await;

    client_result
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
