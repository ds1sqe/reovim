//! TUI platform `run(args)` entry point.
//!
//! Dispatches to headless or interactive TUI based on `TuiArgs`.
//! Integrated-mode composition (`run_integrated`) is NOT here; an
//! embedded-launcher crate will own that path once it exists.
//!
//! CLM v7 locks `ext/client/platforms/<p>::run(args)` as the bin's
//! dispatch target — the standalone `reovim-tui` bin and any future
//! in-process launcher both drive the TUI through [`run`].
//!
//! Transport dispatch (#769 2b.G):
//!
//! - `TuiArgs.grpc` = `"host:port"` — TCP. Delegates to the TUI's
//!   built-in `TuiGrpcClient::connect` path.
//! - `TuiArgs.grpc` = `"uds://<path>"` — Unix-domain socket. Builds a
//!   `tonic::transport::Channel` via `Endpoint::connect_with_connector`
//!   backed by a `tokio::net::UnixStream`; `#[cfg(unix)]` only.
//! - In-process `DuplexStream` — pre-connected stream handed to
//!   [`run_with_stream`] by an embedded launcher. Bypasses `TuiArgs.grpc`
//!   entirely.

use std::{collections::HashSet, io, path::PathBuf};

use {
    clap::Args,
    http::Uri,
    hyper_util::rt::TokioIo,
    reovim_client_tui::{
        TuiGrpcClient, connect_headless_with_client, connect_interactive_with_client,
    },
    tonic::transport::{Channel, Endpoint},
    tower::service_fn,
};

use crate::{TuiAppError, connect_headless, connect_interactive};

/// CLI arguments for the standalone TUI platform entry point.
///
/// `apps/tui/src/main.rs` uses this type directly as its clap-parsed
/// argument; an in-process launcher constructs it in code to share one
/// schema with the standalone bin.
#[derive(Args, Debug, Clone)]
pub struct TuiArgs {
    /// gRPC server address.
    ///
    /// Accepts either a TCP `host:port` (default
    /// `"127.0.0.1:12540"`) or a `"uds://<path>"` prefix selecting the
    /// Unix-domain-socket connector. The in-process duplex-stream path
    /// bypasses this field; see [`run_with_stream`].
    #[arg(long, default_value = "127.0.0.1:12540")]
    pub grpc: String,

    /// Run in headless mode (no TTY, for scripting).
    #[arg(long)]
    pub headless: bool,

    /// Viewport width (headless mode only; interactive mode auto-detects).
    #[arg(long, default_value = "120")]
    pub width: u16,

    /// Viewport height (headless mode only; interactive mode auto-detects).
    #[arg(long, default_value = "40")]
    pub height: u16,

    /// Log file path. When unset, logs go to stderr.
    #[arg(long, value_name = "PATH")]
    pub log: Option<PathBuf>,
}

impl Default for TuiArgs {
    fn default() -> Self {
        Self {
            grpc: "127.0.0.1:12540".to_string(),
            headless: false,
            width: 120,
            height: 40,
            log: None,
        }
    }
}

/// Error type returned by the TUI platform `run(args)` entry point.
///
/// Collapses to an `io::Error` via the `From<TuiRunError> for io::Error`
/// impl so the bin-level `apps/tui/src/lib.rs::run` can propagate with
/// `?` into its `io::Result<()>` signature.
#[derive(Debug, thiserror::Error)]
pub enum TuiRunError {
    /// Connection to the gRPC server failed.
    #[error("failed to connect TUI to {addr}: {source}")]
    Connect {
        /// Target gRPC server address.
        addr: String,
        /// Underlying connection error from the TUI app layer.
        #[source]
        source: TuiAppError,
    },

    /// Non-TCP transport (UDS / in-process) failed to build the
    /// `tonic::transport::Channel` before the handshake began.
    #[error("failed to build gRPC channel for {addr}: {source}")]
    Channel {
        /// Transport-typed display string (e.g. `"uds:///tmp/foo.sock"`
        /// or `"inproc://duplex"`).
        addr: String,
        /// Underlying tonic transport error.
        #[source]
        source: tonic::transport::Error,
    },

    /// The TUI event loop itself failed.
    #[error("TUI app error: {0}")]
    App(#[source] TuiAppError),

    /// An I/O error occurred outside the app layer (e.g. signal handler).
    #[error(transparent)]
    Io(#[from] io::Error),
}

impl From<TuiRunError> for io::Error {
    fn from(err: TuiRunError) -> Self {
        match err {
            TuiRunError::Io(e) => e,
            TuiRunError::Connect { source, .. } => {
                Self::new(io::ErrorKind::ConnectionRefused, source.to_string())
            }
            TuiRunError::Channel { source, .. } => {
                Self::new(io::ErrorKind::ConnectionRefused, source.to_string())
            }
            TuiRunError::App(source) => Self::other(source.to_string()),
        }
    }
}

/// Prefix that marks a `TuiArgs.grpc` string as a UDS path.
const UDS_PREFIX: &str = "uds://";

/// Run the TUI platform against an already-running gRPC server.
///
/// Dispatches by `args.grpc` prefix:
/// - `"uds://<path>"` → Unix-domain socket connector (Unix only).
/// - anything else → TCP (delegates to the TUI's built-in TCP path).
///
/// Further dispatches to headless or interactive mode based on
/// `args.headless`. The caller is responsible for initializing logging
/// (see [`crate::logging::init`]) and supplying a tokio runtime.
///
/// # Errors
///
/// Returns [`TuiRunError::Connect`] if the gRPC handshake fails,
/// [`TuiRunError::Channel`] if a non-TCP channel fails to build,
/// [`TuiRunError::App`] if the TUI event loop errors, or
/// [`TuiRunError::Io`] if the ctrl-c handler or terminal I/O fails.
pub async fn run(args: TuiArgs) -> Result<(), TuiRunError> {
    // Standalone TUI has no server-side context, so the disabled-
    // extension set is empty. An in-process launcher would compute
    // this set from its bootstrap state and pass it in through a
    // richer entry.
    let disabled: HashSet<String> = HashSet::new();

    // Wave 3a (#771): consult `pkg.lock` if present, eager-filter the
    // driver scan, and fan out `on-capability` triggers for the static
    // `PROVIDED_CAPABILITY_NAMES` list. The loaded drivers and the
    // hook are held to keep their cdylibs mapped; wiring them into
    // the live TUI render path is a follow-up flight.
    let _packaged = load_packaged_drivers();

    if let Some(path) = args.grpc.strip_prefix(UDS_PREFIX) {
        return run_over_uds(path, &args, &disabled).await;
    }

    if args.headless {
        run_headless_tcp(&args.grpc, args.width, args.height, &disabled).await
    } else {
        run_interactive_tcp(&args.grpc, &disabled).await
    }
}

/// Run the TUI platform over a pre-built in-process [`tokio::io::DuplexStream`].
///
/// The embedded launcher owns the server-side duplex half (handed to
/// `Server::run_inproc`) and calls this function with the client-side
/// half. The stream becomes the transport for a
/// `tonic::transport::Channel` built via
/// `Endpoint::connect_with_connector` + `TokioIo`.
///
/// `args.grpc` is ignored (the duplex stream is the transport); all
/// other `TuiArgs` fields still apply (`headless`, `width`, `height`,
/// `log`). Logging is the caller's responsibility — the embedded
/// launcher owns terminal I/O.
///
/// # Errors
///
/// See [`run`] for the error taxonomy. `TuiRunError::Channel` is the
/// likely failure mode if the stream is closed before the preface.
pub async fn run_with_stream(
    stream: tokio::io::DuplexStream,
    args: TuiArgs,
) -> Result<(), TuiRunError> {
    let display_hint = "inproc://duplex";
    let disabled: HashSet<String> = HashSet::new();

    // LIFECYCLE: wrap the duplex half in Option + Mutex so the connector
    // closure (which must be Fn, not FnOnce) can move the stream out on
    // the first (and only) invocation. tonic's HTTP/2 transport opens
    // exactly one connection over the channel, so a second call would
    // mean something is wrong — return a closed error instead of
    // panicking.
    let stream_slot = std::sync::Arc::new(std::sync::Mutex::new(Some(stream)));
    let channel = channel_from_connector(display_hint, move |_: Uri| {
        let stream_slot = std::sync::Arc::clone(&stream_slot);
        async move {
            let mut slot = stream_slot
                .lock()
                .map_err(|_| io::Error::other("duplex slot mutex poisoned"))?;
            slot.take()
                .map(TokioIo::new)
                .ok_or_else(|| io::Error::other("duplex stream already consumed"))
        }
    })
    .await?;

    dispatch_mode_with_channel(channel, display_hint, &args, &disabled).await
}

/// Headless TCP path: preserves the TUI's built-in TCP connect.
#[cfg_attr(coverage_nightly, coverage(off))]
async fn run_headless_tcp(
    addr: &str,
    width: u16,
    height: u16,
    disabled: &HashSet<String>,
) -> Result<(), TuiRunError> {
    tracing::info!("Connecting headless TUI to {addr} ({width}x{height})");

    let (mut app, handle) = connect_headless(addr, width, height, None, None, disabled)
        .await
        .map_err(|source| TuiRunError::Connect {
            addr: addr.to_string(),
            source,
        })?;

    tracing::info!("Headless TUI connected and running");

    let app_handle = tokio::spawn(async move { app.run().await });

    tokio::signal::ctrl_c().await?;
    handle.stop().await;

    // Ignore JoinError — event loop already torn down by handle.stop().
    let _ = app_handle.await;

    Ok(())
}

/// Interactive TCP path: preserves the TUI's built-in TCP connect.
#[cfg_attr(coverage_nightly, coverage(off))]
async fn run_interactive_tcp(addr: &str, disabled: &HashSet<String>) -> Result<(), TuiRunError> {
    tracing::info!("Connecting interactive TUI to {addr}");

    let (mut app, _handle) = connect_interactive(addr, None, None, disabled)
        .await
        .map_err(|source| TuiRunError::Connect {
            addr: addr.to_string(),
            source,
        })?;

    let result = app.run().await.map_err(TuiRunError::App);
    drop(app);
    result
}

/// Run the TUI over a Unix-domain-socket-backed gRPC channel.
#[cfg(unix)]
#[cfg_attr(coverage_nightly, coverage(off))]
async fn run_over_uds(
    path: &str,
    args: &TuiArgs,
    disabled: &HashSet<String>,
) -> Result<(), TuiRunError> {
    let display_hint = format!("{UDS_PREFIX}{path}");
    let owned_path = PathBuf::from(path);
    let channel = channel_from_connector(&display_hint, move |_: Uri| {
        let p = owned_path.clone();
        async move { tokio::net::UnixStream::connect(&p).await.map(TokioIo::new) }
    })
    .await?;

    dispatch_mode_with_channel(channel, &display_hint, args, disabled).await
}

/// Fallback for non-unix builds: surface the OS-specific refusal as a
/// channel-build error instead of a silent no-op.
#[cfg(not(unix))]
#[cfg_attr(coverage_nightly, coverage(off))]
async fn run_over_uds(
    path: &str,
    _args: &TuiArgs,
    _disabled: &HashSet<String>,
) -> Result<(), TuiRunError> {
    Err(TuiRunError::Io(io::Error::new(
        io::ErrorKind::Unsupported,
        format!("uds://{path}: Unix-domain sockets are not supported on this platform"),
    )))
}

/// Build a `tonic::transport::Channel` from a single-connection
/// connector closure.
///
/// The `display_hint` is used only in error messages — the real
/// transport is dictated by `make_conn`. The `Endpoint` needs a
/// parseable URI; we pass a dummy `http://[::]:50051` per the tonic
/// UDS example — the URI is inert once `connect_with_connector` takes
/// over.
async fn channel_from_connector<C, F, IO>(
    display_hint: &str,
    make_conn: C,
) -> Result<Channel, TuiRunError>
where
    C: FnMut(Uri) -> F + Send + 'static,
    F: std::future::Future<Output = Result<IO, io::Error>> + Send + 'static,
    IO: hyper::rt::Read + hyper::rt::Write + Send + Unpin + 'static,
{
    // `from_static` is infallible for this compile-time string; the
    // URI is inert once `connect_with_connector` takes over.
    Endpoint::from_static("http://[::]:50051")
        .connect_with_connector(service_fn(make_conn))
        .await
        .map_err(|source| TuiRunError::Channel {
            addr: display_hint.to_string(),
            source,
        })
}

/// Dispatch the TUI's headless/interactive mode given a ready channel.
///
/// Shared between the UDS arm of [`run`] and [`run_with_stream`]. The
/// display hint surfaces in `TuiApp`'s addr field for diagnostics.
#[cfg_attr(coverage_nightly, coverage(off))]
async fn dispatch_mode_with_channel(
    channel: Channel,
    display_hint: &str,
    args: &TuiArgs,
    disabled: &HashSet<String>,
) -> Result<(), TuiRunError> {
    if args.headless {
        tracing::info!(
            "Connecting headless TUI over {display_hint} ({}x{})",
            args.width,
            args.height,
        );
        let (mut app, handle) = connect_headless_with_client(
            TuiGrpcClient::from_channel(channel),
            display_hint,
            args.width,
            args.height,
            None,
            None,
            disabled,
        )
        .await
        .map_err(|source| TuiRunError::Connect {
            addr: display_hint.to_string(),
            source,
        })?;

        tracing::info!("Headless TUI connected and running");
        let app_handle = tokio::spawn(async move { app.run().await });
        tokio::signal::ctrl_c().await?;
        handle.stop().await;
        let _ = app_handle.await;
        Ok(())
    } else {
        tracing::info!("Connecting interactive TUI over {display_hint}");
        let (mut app, _handle) = connect_interactive_with_client(
            TuiGrpcClient::from_channel(channel),
            display_hint,
            None,
            None,
            disabled,
        )
        .await
        .map_err(|source| TuiRunError::Connect {
            addr: display_hint.to_string(),
            source,
        })?;
        let result = app.run().await.map_err(TuiRunError::App);
        drop(app);
        result
    }
}

// ============================================================================
// Wave 3a (#771): packaged-driver lazy load
// ============================================================================

/// Capabilities the TUI platform provides, fanned out at startup so
/// any package whose `pkg.lock` `on-capability = "<name>"` trigger
/// matches one of these names is dlopen'd.
const PROVIDED_CAPABILITY_NAMES: &[&str] = &["cell"];

/// Bundle of resources from a successful packaged-driver load.
///
/// Held by the platform runtime to keep loaded cdylibs mapped for
/// the process lifetime. The follow-up flight that wires these into
/// the live TUI render path will read each field separately.
struct PackagedDrivers {
    /// Eagerly-loaded render drivers, one entry per cdylib that
    /// passed the eager filter.
    _eager_render: Vec<
        Result<
            reovim_client_subsys_driver_loader::LoadedClientRender,
            reovim_client_subsys_driver_loader::ScanEntryError,
        >,
    >,
    /// Eagerly-loaded debug drivers.
    _eager_debug: Vec<
        Result<
            reovim_client_subsys_driver_loader::LoadedClientDebug,
            reovim_client_subsys_driver_loader::ScanEntryError,
        >,
    >,
    /// Lazy hook holding deferred drivers loaded via the boot-time
    /// capability fan-out.
    _lazy_hook: reovim_client_subsys_driver_loader::CapabilityLazyHook,
}

/// Eager-filter the package-manager driver scan and fan out
/// `on-capability` triggers.
///
/// Returns `None` when no library root resolves (the platform then
/// boots without packaged drivers). The returned bundle is held
/// alive at the call site so its cdylibs stay mapped — see
/// [`PackagedDrivers`].
fn load_packaged_drivers() -> Option<PackagedDrivers> {
    let library_root = reovim_pkg_runtime_loader::resolve_library_root()?;
    let registry = match reovim_pkg_runtime_loader::load_registry(&library_root) {
        Ok(reg) => reg,
        Err(err) => {
            tracing::warn!(?err, "could not load runtime LazyRegistry; falling back to empty");
            std::sync::Arc::new(reovim_pkg_lazyload::LazyRegistry::empty())
        }
    };

    let eager_render =
        reovim_client_subsys_driver_loader::LoadedClientRender::from_path_scan_filtered(
            &library_root,
            &registry,
        );
    let eager_debug =
        reovim_client_subsys_driver_loader::LoadedClientDebug::from_path_scan_filtered(
            &library_root,
            &registry,
        );

    let lazy_hook = reovim_client_subsys_driver_loader::CapabilityLazyHook::new(
        std::sync::Arc::clone(&registry),
        library_root,
    );
    for cap in PROVIDED_CAPABILITY_NAMES {
        if let Err(err) = lazy_hook.dispatch_capability(cap) {
            tracing::warn!(capability = cap, ?err, "lazy capability dispatch failed");
        }
    }

    // TODO(#771): Wave-3b — thread the eager driver vectors and the
    // lazy hook through into the live TUI render path.
    Some(PackagedDrivers {
        _eager_render: eager_render,
        _eager_debug: eager_debug,
        _lazy_hook: lazy_hook,
    })
}
