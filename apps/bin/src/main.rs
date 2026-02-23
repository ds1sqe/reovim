#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! Reovim - the main entry point binary.
//!
//! # Usage
//!
//! ```bash
//! # Default: integrated mode (server + TUI in one process)
//! reovim
//!
//! # Standalone server with gRPC transport
//! reovim server --grpc 12540
//!
//! # Start with specific TCP port
//! reovim server --tcp 12522
//!
//! # Start with Unix socket (Unix only)
//! reovim server --socket /tmp/reovim.sock
//!
//! # Connect interactive TUI to existing server
//! reovim tui --grpc 127.0.0.1:12540
//!
//! # Connect headless TUI (for scripting/testing)
//! reovim tui --grpc 127.0.0.1:12540 --headless
//! ```
//!
//! # Key Resolution Flow
//!
//! With module bootstrap enabled, keys are resolved through the full vim resolver system:
//! 1. `InputService::send_keys()` receives key notation
//! 2. For each key, `SessionState::resolve_key()` is called
//! 3. The `ResolverRegistry` finds the appropriate mode resolver (e.g., `VimNormalResolver`)
//! 4. The resolver returns a `ResolveResult` (execute, insert, transition, etc.)
//! 5. The result is handled (command execution, mode push/pop, char insertion)

mod bootstrap;

use {
    clap::{Parser, Subcommand},
    reovim_server::{Server, ServerConfig, TransportMode},
};

use {
    reovim_client_cli::OutputFormat,
    reovim_client_tui::{connect_headless, connect_interactive},
};

/// Reovim editor.
#[derive(Parser)]
#[command(name = "reovim")]
#[command(version, about, long_about = None)]
struct Cli {
    /// Subcommand to run.
    #[command(subcommand)]
    command: Option<Commands>,

    /// Enable verbose logging (debug level).
    #[arg(short, long, global = true)]
    verbose: bool,
}

/// Available subcommands.
#[derive(Subcommand)]
enum Commands {
    /// Start the server.
    Server {
        /// TCP port to listen on.
        #[arg(long, value_name = "PORT")]
        tcp: Option<u16>,

        /// gRPC port to listen on.
        #[arg(long, value_name = "PORT")]
        grpc: Option<u16>,

        /// Unix socket path to listen on.
        #[cfg(unix)]
        #[arg(long, value_name = "PATH")]
        socket: Option<std::path::PathBuf>,

        /// Name for the default session.
        #[arg(long, default_value = "main")]
        session: String,

        /// Instance name for discovery.
        #[arg(long, default_value = "default")]
        instance: String,
    },

    /// Execute CLI commands (gRPC v2).
    Cli {
        /// gRPC server address (host:port).
        #[arg(long, default_value = "127.0.0.1:12540")]
        grpc: String,

        /// Output format.
        #[arg(long, short, value_enum, default_value = "plain")]
        format: CliOutputFormat,

        /// CLI command.
        #[command(subcommand)]
        command: CliSubcommand,
    },

    /// Connect TUI to server (gRPC v2).
    Tui {
        /// gRPC server address (host:port).
        #[arg(long, default_value = "127.0.0.1:12540")]
        grpc: String,

        /// Run in headless mode (no TTY, for scripting).
        #[arg(long)]
        headless: bool,

        /// Viewport width.
        #[arg(long, default_value = "120")]
        width: u16,

        /// Viewport height.
        #[arg(long, default_value = "40")]
        height: u16,
    },
}

/// CLI output format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
enum CliOutputFormat {
    /// Plain text output.
    Plain,
    /// JSON output.
    Json,
}

/// CLI subcommands.
#[derive(Debug, Subcommand)]
enum CliSubcommand {
    /// Send keys to a specific client.
    Keys {
        /// Keys in vim notation.
        keys: String,
        /// Target client ID to send keys to (required).
        #[arg(long, short)]
        client: u64,
    },
    /// Get a specific client's editor mode.
    Mode {
        /// Target client ID to query mode from (required).
        #[arg(long, short)]
        client: u64,
    },
    /// Get a specific client's cursor position.
    Cursor {
        /// Target client ID to query cursor from (required).
        #[arg(long, short)]
        client: u64,
    },
    /// List open buffers.
    Buffers,
    /// Get buffer content.
    Buffer {
        #[arg(long)]
        id: Option<u64>,
    },
    /// Get register contents.
    Registers { name: Option<String> },
    /// Capture TUI screen content.
    Capture {
        /// Target client ID to capture from (required).
        #[arg(long, short)]
        client: u64,
        /// Capture format: `plain_text`, `raw_ansi` (default), `cell_grid`.
        #[arg(long, short = 'f', default_value = "raw_ansi")]
        capture_format: String,
    },
    /// Ping the server.
    Ping,
    /// Get server version and info.
    Version,
    /// Get recent log entries from server ring buffer.
    LogTail {
        /// Number of entries (default: 50).
        #[arg(long, short = 'n', default_value = "50")]
        count: u32,
        /// Filter by level (trace, debug, info, warn, error).
        #[arg(long)]
        level: Option<String>,
        /// Filter by target module.
        #[arg(long)]
        target: Option<String>,
        /// Search in messages (case-insensitive).
        #[arg(long)]
        grep: Option<String>,
    },
    /// List connected clients (read-only debug query).
    Clients,
    /// Query extension state (e.g., which-key, cmdline).
    ExtensionState {
        /// Extension kind to query (e.g., "whichkey", "cmdline").
        kind: String,
        /// Target client ID.
        #[arg(long, short)]
        client: u64,
    },
    /// List registered extensions.
    Extensions,
}

#[cfg_attr(coverage_nightly, coverage(off))]
fn main() -> std::io::Result<()> {
    let cli = Cli::parse();

    // Initialize tracing
    // Priority: RUST_LOG env var > --verbose flag > default (info)
    // Note: We use stderr so test harness can capture logs (stdout is piped to null)
    let filter = std::env::var("RUST_LOG").unwrap_or_else(|_| {
        if cli.verbose {
            "debug".to_string()
        } else {
            "info".to_string()
        }
    });
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(&filter)
        .init();

    // Initialize debug infrastructure (Phase #478)
    init_debug_infrastructure();

    // Run async runtime
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?
        .block_on(run(cli))
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

#[cfg_attr(coverage_nightly, coverage(off))]
async fn run(cli: Cli) -> std::io::Result<()> {
    match cli.command {
        Some(Commands::Server {
            tcp,
            grpc,
            #[cfg(unix)]
            socket,
            session,
            instance,
        }) => {
            // Determine transport mode based on arguments
            let transport = determine_transport(
                tcp,
                grpc,
                #[cfg(unix)]
                socket,
            );

            let config = ServerConfig {
                transport,
                instance_name: instance,
                default_session_name: session,
            };

            tracing::info!("Starting reovim server (new architecture with modules)");

            // Create server with module-initialized session factory and bridges
            let bridges = bootstrap::create_bridge_registry();
            let server =
                Server::with_session_factory(config, Box::new(bootstrap::create_session_state))
                    .with_bridges(bridges);
            server.run().await
        }

        Some(Commands::Cli {
            grpc,
            format,
            command,
        }) => run_cli(&grpc, format, command).await,

        Some(Commands::Tui {
            grpc,
            headless,
            width,
            height,
        }) => {
            if headless {
                run_headless_tui(&grpc, width, height).await
            } else {
                run_interactive_tui(&grpc).await
            }
        }

        None => run_integrated().await,
    }
}

/// Run CLI command.
#[cfg_attr(coverage_nightly, coverage(off))]
async fn run_cli(
    addr: &str,
    format: CliOutputFormat,
    command: CliSubcommand,
) -> std::io::Result<()> {
    use reovim_client_cli::{GrpcClient, GrpcClientError, commands};

    let output_format = match format {
        CliOutputFormat::Plain => OutputFormat::Plain,
        CliOutputFormat::Json => OutputFormat::Json,
    };

    let mut client = GrpcClient::connect(addr)
        .await
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::ConnectionRefused, e.to_string()))?;

    let result: Result<String, GrpcClientError> = match command {
        CliSubcommand::Keys {
            keys,
            client: target,
        } => commands::keys(&mut client, &keys, target, output_format).await,
        CliSubcommand::Mode { client: target } => {
            commands::mode(&mut client, target, output_format).await
        }
        CliSubcommand::Cursor { client: target } => {
            commands::cursor(&mut client, target, output_format).await
        }
        CliSubcommand::Buffers => commands::buffers(&mut client, output_format).await,
        CliSubcommand::Buffer { id } => commands::buffer(&mut client, id, output_format).await,
        CliSubcommand::Registers { name } => {
            commands::registers(&mut client, name, output_format).await
        }
        CliSubcommand::Capture {
            client: client_id,
            capture_format,
        } => commands::capture(&mut client, client_id, &capture_format, output_format).await,
        CliSubcommand::Ping => commands::ping(&mut client, output_format).await,
        CliSubcommand::Version => commands::version(&mut client, output_format).await,
        CliSubcommand::LogTail {
            count,
            level,
            target,
            grep,
        } => commands::log_tail(&mut client, count, level, target, grep, output_format).await,
        CliSubcommand::Clients => commands::clients(&mut client, output_format).await,
        CliSubcommand::ExtensionState {
            kind,
            client: target,
        } => commands::extension_state(&mut client, &kind, target, output_format).await,
        CliSubcommand::Extensions => commands::extensions(&mut client, output_format).await,
    };
    drop(client); // Release gRPC connection early

    match result {
        Ok(output) => {
            println!("{output}");
            Ok(())
        }
        Err(e) => Err(std::io::Error::other(e.to_string())),
    }
}

/// Run headless TUI.
#[cfg_attr(coverage_nightly, coverage(off))]
async fn run_headless_tui(addr: &str, width: u16, height: u16) -> std::io::Result<()> {
    tracing::info!("Connecting headless TUI to {addr} ({width}x{height})");

    let (mut app, handle) = connect_headless(addr, width, height, None, None)
        .await
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::ConnectionRefused, e.to_string()))?;

    tracing::info!("Headless TUI connected and running");

    // Spawn event loop and wait for Ctrl-C
    let app_handle = tokio::spawn(async move { app.run().await });

    tokio::signal::ctrl_c().await?;
    handle.stop().await;

    // Wait for event loop to finish
    let _ = app_handle.await;

    Ok(())
}

/// Run interactive TUI.
#[cfg_attr(coverage_nightly, coverage(off))]
async fn run_interactive_tui(addr: &str) -> std::io::Result<()> {
    tracing::info!("Connecting interactive TUI to {addr}");

    let (mut app, _handle) = connect_interactive(addr, None, None)
        .await
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::ConnectionRefused, e.to_string()))?;

    let result = app
        .run()
        .await
        .map_err(|e| std::io::Error::other(e.to_string()));

    drop(app);
    result
}

/// Run in integrated mode: server + interactive TUI in one process.
///
/// The server binds to an OS-assigned port, then the TUI connects to it.
/// When the TUI exits (Ctrl-Q or `:q`), the server shuts down gracefully.
/// Ctrl-C also stops the TUI, which then triggers server shutdown.
#[cfg_attr(coverage_nightly, coverage(off))]
async fn run_integrated() -> std::io::Result<()> {
    tracing::info!("Starting reovim in integrated mode (server + TUI)");

    // Shutdown channel: completing the future signals the server to stop
    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();

    // Port channel: server reports its OS-assigned port
    let (port_tx, port_rx) = tokio::sync::oneshot::channel::<u16>();

    // Configure server with OS-assigned port (port 0)
    let config = ServerConfig {
        transport: TransportMode::Grpc { port: 0 },
        instance_name: "default".to_string(),
        default_session_name: "main".to_string(),
    };
    let bridges = bootstrap::create_bridge_registry();
    let server = Server::with_session_factory(config, Box::new(bootstrap::create_session_state))
        .with_bridges(bridges);

    // Spawn server task
    let server_task = tokio::spawn(async move {
        let shutdown = async {
            let _ = shutdown_rx.await;
        };
        server.run_until(shutdown, Some(port_tx)).await
    });

    // Wait for port with timeout
    let port = tokio::time::timeout(std::time::Duration::from_secs(10), port_rx)
        .await
        .map_err(|_| {
            std::io::Error::new(std::io::ErrorKind::TimedOut, "Server failed to start within 10s")
        })?
        .map_err(|_| {
            std::io::Error::new(
                std::io::ErrorKind::BrokenPipe,
                "Server exited before reporting port",
            )
        })?;

    let addr = format!("127.0.0.1:{port}");
    tracing::info!("Server listening on {addr}, connecting TUI...");

    // Connect interactive TUI
    let (mut app, handle) = connect_interactive(&addr, None, None)
        .await
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::ConnectionRefused, e.to_string()))?;

    // Ctrl-C handler: gracefully stop TUI
    let ctrl_c_handle = handle.clone();
    tokio::spawn(async move {
        if tokio::signal::ctrl_c().await.is_ok() {
            ctrl_c_handle.stop().await;
        }
    });

    // Run TUI (blocks until user quits or Ctrl-C)
    let result = app
        .run()
        .await
        .map_err(|e| std::io::Error::other(e.to_string()));
    drop(app);

    // Signal server shutdown
    let _ = shutdown_tx.send(());

    // Wait for server to finish (5s timeout)
    let _ = tokio::time::timeout(std::time::Duration::from_secs(5), server_task).await;

    result
}

/// Determine transport mode from CLI arguments.
#[allow(unused_variables)]
fn determine_transport(
    tcp: Option<u16>,
    grpc: Option<u16>,
    #[cfg(unix)] socket: Option<std::path::PathBuf>,
) -> TransportMode {
    // Priority: gRPC > Unix socket > TCP > TCP fallback
    if let Some(port) = grpc {
        return TransportMode::Grpc { port };
    }

    #[cfg(unix)]
    if let Some(path) = socket {
        return TransportMode::UnixSocket { path };
    }

    if let Some(port) = tcp {
        return TransportMode::Tcp { port };
    }

    TransportMode::TcpWithFallback
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_determine_transport_grpc() {
        let result = determine_transport(
            None,
            Some(12540),
            #[cfg(unix)]
            None,
        );
        assert!(matches!(result, TransportMode::Grpc { port: 12540 }));
    }

    #[test]
    fn test_determine_transport_tcp() {
        let result = determine_transport(
            Some(12522),
            None,
            #[cfg(unix)]
            None,
        );
        assert!(matches!(result, TransportMode::Tcp { port: 12522 }));
    }

    #[test]
    fn test_determine_transport_fallback() {
        let result = determine_transport(
            None,
            None,
            #[cfg(unix)]
            None,
        );
        assert!(matches!(result, TransportMode::TcpWithFallback));
    }

    #[test]
    fn test_determine_transport_grpc_over_tcp() {
        // gRPC takes priority over TCP
        let result = determine_transport(
            Some(12522),
            Some(12540),
            #[cfg(unix)]
            None,
        );
        assert!(matches!(result, TransportMode::Grpc { port: 12540 }));
    }

    #[cfg(unix)]
    #[test]
    fn test_determine_transport_socket() {
        let result = determine_transport(None, None, Some("/tmp/reovim.sock".into()));
        assert!(matches!(result, TransportMode::UnixSocket { .. }));
    }

    #[cfg(unix)]
    #[test]
    fn test_determine_transport_grpc_over_socket() {
        // gRPC takes priority over socket
        let result = determine_transport(None, Some(12540), Some("/tmp/reovim.sock".into()));
        assert!(matches!(result, TransportMode::Grpc { port: 12540 }));
    }

    #[test]
    fn test_cli_output_format_debug() {
        assert!(format!("{:?}", CliOutputFormat::Plain).contains("Plain"));
        assert!(format!("{:?}", CliOutputFormat::Json).contains("Json"));
    }

    #[test]
    fn test_cli_output_format_clone_eq() {
        let a = CliOutputFormat::Plain;
        let b = a;
        assert_eq!(a, b);
    }

    #[test]
    fn test_cli_extension_state_parse() {
        let cli = Cli::parse_from([
            "reovim",
            "cli",
            "extension-state",
            "whichkey",
            "--client",
            "1",
        ]);
        match cli.command {
            Some(Commands::Cli { command, .. }) => {
                assert!(matches!(
                    command,
                    CliSubcommand::ExtensionState { ref kind, client: 1 } if kind == "whichkey"
                ));
            }
            _ => panic!("Expected Cli command"),
        }
    }

    #[test]
    fn test_cli_extensions_parse() {
        let cli = Cli::parse_from(["reovim", "cli", "extensions"]);
        match cli.command {
            Some(Commands::Cli { command, .. }) => {
                assert!(matches!(command, CliSubcommand::Extensions));
            }
            _ => panic!("Expected Cli command"),
        }
    }
}
