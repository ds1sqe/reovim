#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! Standalone reovim-server library.
//!
//! Hosts the server-side composition root: bootstrap, logging, profiling,
//! and the runner-side module gRPC service. The `bin` target is a thin
//! clap + tokio wrapper around [`run`]; an in-process launcher drives
//! the server through the same [`run`] entry.
//!
//! # Usage
//!
//! ```bash
//! # Start the server with gRPC transport on port 12540
//! reovim-server --grpc 12540
//!
//! # Module management (offline; no running server required)
//! reovim-server module list
//! ```

use std::sync::Arc;

use {
    clap::{Parser, Subcommand, ValueEnum},
    reovim_server::{Server, ServerConfig, TransportMode},
};

pub mod bootstrap;
pub mod logging;
pub mod module_cli;
pub mod module_service;
pub mod profiling;

#[cfg(test)]
#[path = "lib_tests.rs"]
mod lib_tests;

/// Top-level CLI for the standalone `reovim-server` binary.
///
/// Accepts server transport flags directly (default action: start the
/// server on gRPC port 12540) and optionally a `module` subcommand for
/// offline module lifecycle operations that do not require a running
/// server.
#[derive(Parser, Debug)]
#[command(name = "reovim-server")]
#[command(version, about, long_about = None)]
pub struct ServerArgs {
    /// TCP port to listen on.
    #[arg(long, value_name = "PORT", global = true)]
    pub tcp: Option<u16>,

    /// gRPC port to listen on (default: 12540 when no transport flag is set).
    #[arg(long, value_name = "PORT", global = true)]
    pub grpc: Option<u16>,

    /// Unix socket path to listen on.
    #[cfg(unix)]
    #[arg(long, value_name = "PATH", global = true)]
    pub socket: Option<std::path::PathBuf>,

    /// Name for the default session.
    #[arg(long, default_value = "main", global = true)]
    pub session: String,

    /// Instance name for discovery.
    #[arg(long, default_value = "default", global = true)]
    pub instance: String,

    /// Enable verbose logging (debug level).
    #[arg(short, long, global = true)]
    pub verbose: bool,

    /// Log file path (default: stderr for server/module mode).
    #[arg(long, global = true, value_name = "PATH")]
    pub log: Option<std::path::PathBuf>,

    /// Transport kind for stream-based transports that do not fit the
    /// port-style flags above. `pipe` wires the server's gRPC stack
    /// over `stdin`/`stdout`; `inproc` is reserved for the embedded
    /// launcher and is rejected at the standalone bin.
    #[arg(long, value_name = "KIND", global = true)]
    pub transport: Option<TransportArg>,

    /// Optional subcommand (e.g. `module`). If omitted, starts the server.
    #[command(subcommand)]
    pub command: Option<ServerCommand>,
}

/// CLI spelling of stream-based transport kinds accepted by the
/// standalone `reovim-server` bin's `--transport` flag.
#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum TransportArg {
    /// Read from stdin, write to stdout.
    Pipe,
    /// Reserved for the embedded launcher; rejected at the standalone bin.
    Inproc,
}

/// Subcommands under `reovim-server`.
#[derive(Subcommand, Debug)]
pub enum ServerCommand {
    /// Manage third-party modules (install, remove, update).
    Module {
        /// Module management subcommand.
        #[command(subcommand)]
        command: module_cli::ModuleCommand,
    },
}

/// Initialize tracing subscriber and debug infrastructure for server mode.
///
/// Priority for log filter: `RUST_LOG` env var > `--verbose` flag > `info`.
/// Server mode logs to stderr by default (file logging only if `--log` is set).
#[cfg_attr(coverage_nightly, coverage(off))]
fn init_tracing(verbose: bool, log: Option<&std::path::Path>) {
    let filter = std::env::var("RUST_LOG").unwrap_or_else(|_| {
        if verbose {
            "debug".to_string()
        } else {
            "info".to_string()
        }
    });

    if let Some(log_path) = log {
        if let Some(parent) = log_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let config = logging::LogConfig {
            level: if verbose {
                reovim_kernel::api::v1::Level::Debug
            } else {
                reovim_kernel::api::v1::Level::Info
            },
            output: logging::LogOutput::File,
            format: logging::LogFormat::Plain,
            file_path: Some(log_path.to_path_buf()),
            rotation: logging::RotationPolicy::Never,
        };
        if let Err(e) = logging::init_logging(&config) {
            eprintln!("Failed to init file logging: {e}, falling back to stderr");
            tracing_subscriber::fmt()
                .with_writer(std::io::stderr)
                .with_env_filter(&filter)
                .init();
        }
    } else {
        tracing_subscriber::fmt()
            .with_writer(std::io::stderr)
            .with_env_filter(&filter)
            .init();
    }
}

/// Initialize debug infrastructure for crash reports.
///
/// Sets up:
/// 1. Server debug ring buffer (64 KB)
/// 2. Composite logger (writes to ring buffer + tracing)
/// 3. Debug context callback for panic handler
/// 4. Custom panic handler
#[cfg_attr(coverage_nightly, coverage(off))]
fn init_debug_infrastructure() {
    use {
        reovim_kernel::api::v1::{DebugContext, install_panic_handler, set_debug_context_callback},
        reovim_server::debug::{
            COMPOSITE_LOGGER, DebugRingBuffer, init_debug_ring, try_debug_ring,
        },
    };

    // 1. Initialize global debug ring buffer
    if let Err(e) = init_debug_ring() {
        tracing::warn!("Debug ring buffer already initialized: {e}");
    }

    // 2. Set composite logger (ring buffer + tracing passthrough)
    if let Err(e) = reovim_kernel::api::v1::set_logger(&COMPOSITE_LOGGER) {
        tracing::warn!("Logger already set: {e}");
    }

    // 2b. Set tracing profiler so profile_scope! emits flame-graph spans
    // (no-op unless REOVIM_PROFILE is set).
    if let Err(e) = profiling::init_profiling() {
        tracing::warn!("Profiler already set: {e}");
    }

    // 3. Set debug context callback for panic handler
    set_debug_context_callback(Box::new(|| {
        let server_logs = try_debug_ring().and_then(DebugRingBuffer::try_dump);
        DebugContext {
            server_logs,
            client_dump_paths: Vec::new(),
        }
    }));

    // 4. Install panic handler
    install_panic_handler();

    tracing::debug!("Debug infrastructure initialized");
}

/// Determine transport mode from CLI arguments.
///
/// `--transport pipe` takes priority and maps to `TransportMode::Pipe`.
/// `--transport inproc` is rejected with a pointer at the embedded
/// launcher. Port-style flags follow: gRPC > Unix socket > TCP. When no
/// transport flag is set, the default is gRPC on port 12540, matching
/// the TUI and CLI client defaults so a bare `reovim-server` is
/// immediately connectable without extra flags.
///
/// # Errors
///
/// Returns `InvalidInput` when `--transport inproc` is passed to the
/// standalone bin — inproc has no OS-crossing wire and must be driven
/// by the embedded launcher.
#[allow(unused_variables)]
pub(crate) fn determine_transport(
    tcp: Option<u16>,
    grpc: Option<u16>,
    #[cfg(unix)] socket: Option<std::path::PathBuf>,
    transport: Option<TransportArg>,
) -> std::io::Result<TransportMode> {
    if let Some(kind) = transport {
        return match kind {
            TransportArg::Pipe => Ok(TransportMode::Pipe),
            TransportArg::Inproc => Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "inproc transport requires embedded mode — invoke the reovim launcher \
                 without --subprocess instead of running reovim-server --transport inproc",
            )),
        };
    }

    if let Some(port) = grpc {
        return Ok(TransportMode::Grpc { port });
    }

    #[cfg(unix)]
    if let Some(path) = socket {
        return Ok(TransportMode::UnixSocket { path });
    }

    if let Some(port) = tcp {
        return Ok(TransportMode::Tcp { port });
    }

    Ok(TransportMode::Grpc { port: 12540 })
}

/// Run the standalone `reovim-server` flow.
///
/// Initializes tracing + debug infrastructure, then either dispatches a
/// `module` subcommand or boots the gRPC server runtime.
///
/// # Errors
///
/// Returns an I/O error if tokio runtime setup, server start-up, or a
/// module-management subcommand fails.
#[cfg_attr(coverage_nightly, coverage(off))]
pub async fn run(args: ServerArgs) -> std::io::Result<()> {
    init_tracing(args.verbose, args.log.as_deref());
    init_debug_infrastructure();

    match args.command {
        Some(ServerCommand::Module { command }) => module_cli::run(&command),
        None => run_server(args).await,
    }
}

/// Boot the server runtime using the transport flags on `args`.
#[cfg_attr(coverage_nightly, coverage(off))]
async fn run_server(args: ServerArgs) -> std::io::Result<()> {
    let transport = determine_transport(
        args.tcp,
        args.grpc,
        #[cfg(unix)]
        args.socket,
        args.transport,
    )?;

    let config = ServerConfig {
        transport: transport.clone(),
        instance_name: args.instance,
        default_session_name: args.session,
    };

    tracing::info!("Starting reovim server (new architecture with modules)");

    let bootstrap = bootstrap::bootstrap_runtime();
    let runner_module_service = module_service::RunnerGrpcModuleService::new(
        Arc::clone(&bootstrap.module_registry),
        Arc::clone(&bootstrap.module_ctx),
    );
    let mut server = Server::new(config)
        .with_initial_session_state(bootstrap.session_state)
        .with_bridges(bootstrap.bridges)
        .with_module_service(runner_module_service);
    if let Some(driver) = bootstrap.domain_driver {
        server = server.with_domain_driver(driver);
    }
    match transport {
        TransportMode::Pipe => {
            server
                .run_pipe(tokio::io::stdin(), tokio::io::stdout())
                .await
        }
        _ => server.run().await,
    }
}
