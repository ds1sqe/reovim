//! Reovim - new architecture binary.
//!
//! This is the new runner that uses `lib/server/` directly, implementing
//! the server/client split from Epic #465.
//!
//! # Key Resolution Flow
//!
//! With module bootstrap enabled, keys are resolved through the full vim resolver system:
//! 1. `InputService::send_keys()` receives key notation
//! 2. For each key, `SessionState::resolve_key()` is called
//! 3. The `ResolverRegistry` finds the appropriate mode resolver (e.g., `VimNormalResolver`)
//! 4. The resolver returns a `ResolveResult` (execute, insert, transition, etc.)
//! 5. The result is handled (command execution, mode push/pop, char insertion)
//!
//! # Usage
//!
//! ```bash
//! # Default: start server with TCP fallback (ports 12540-12549)
//! reovim-new
//!
//! # Start with specific TCP port
//! reovim-new server --tcp 12522
//!
//! # Start with gRPC transport (requires --features grpc)
//! reovim-new server --grpc 12540
//!
//! # Start with Unix socket (Unix only)
//! reovim-new server --socket /tmp/reovim.sock
//! ```

mod bootstrap;

use {
    clap::{Parser, Subcommand},
    reovim_server::{Server, ServerConfig, TransportMode},
};

#[cfg(feature = "grpc")]
use reovim_client_cli::OutputFormat;

#[cfg(feature = "grpc")]
use reovim_client_tui::TuiAppV2Headless;

/// Reovim editor - new architecture.
#[derive(Parser)]
#[command(name = "reovim-new")]
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

        /// gRPC port to listen on (requires --features grpc).
        #[cfg(feature = "grpc")]
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
    #[cfg(feature = "grpc")]
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

    /// Connect headless TUI to server (gRPC v2).
    #[cfg(feature = "grpc")]
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
#[cfg(feature = "grpc")]
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
enum CliOutputFormat {
    /// Plain text output.
    Plain,
    /// JSON output.
    Json,
}

/// CLI subcommands.
#[cfg(feature = "grpc")]
#[derive(Debug, Subcommand)]
enum CliSubcommand {
    /// Send keys to the editor.
    Keys {
        /// Keys in vim notation.
        keys: String,
    },
    /// Get current editor mode.
    Mode,
    /// Get cursor position.
    Cursor,
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
        /// Capture format: `plain_text`, `raw_ansi` (default), `cell_grid`.
        #[arg(long, short = 'f', default_value = "raw_ansi")]
        capture_format: String,
    },
    /// Ping the server.
    Ping,
    /// Get server version and info.
    Version,
}

fn main() -> std::io::Result<()> {
    let cli = Cli::parse();

    // Initialize tracing
    let filter = if cli.verbose { "debug" } else { "info" };
    tracing_subscriber::fmt().with_env_filter(filter).init();

    // Run async runtime
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?
        .block_on(run(cli))
}

async fn run(cli: Cli) -> std::io::Result<()> {
    match cli.command {
        Some(Commands::Server {
            tcp,
            #[cfg(feature = "grpc")]
            grpc,
            #[cfg(unix)]
            socket,
            session,
            instance,
        }) => {
            // Determine transport mode based on arguments
            let transport = determine_transport(
                tcp,
                #[cfg(feature = "grpc")]
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

            // Create server with module-initialized session factory
            let server =
                Server::with_session_factory(config, Box::new(bootstrap::create_session_state));
            server.run().await
        }

        #[cfg(feature = "grpc")]
        Some(Commands::Cli {
            grpc,
            format,
            command,
        }) => run_cli(&grpc, format, command).await,

        #[cfg(feature = "grpc")]
        Some(Commands::Tui {
            grpc,
            headless,
            width,
            height,
        }) => {
            if headless {
                run_headless_tui(&grpc, width, height).await
            } else {
                eprintln!("Interactive TUI not implemented yet - use --headless");
                std::process::exit(1);
            }
        }

        None => {
            // Default: start server with TCP fallback and modules
            tracing::info!("Starting reovim server with default configuration and modules");
            let config = ServerConfig::default();
            let server =
                Server::with_session_factory(config, Box::new(bootstrap::create_session_state));
            server.run().await
        }
    }
}

/// Run CLI command.
#[cfg(feature = "grpc")]
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
        CliSubcommand::Keys { keys } => commands::keys(&mut client, &keys, output_format).await,
        CliSubcommand::Mode => commands::mode(&mut client, output_format).await,
        CliSubcommand::Cursor => commands::cursor(&mut client, output_format).await,
        CliSubcommand::Buffers => commands::buffers(&mut client, output_format).await,
        CliSubcommand::Buffer { id } => commands::buffer(&mut client, id, output_format).await,
        CliSubcommand::Registers { name } => {
            commands::registers(&mut client, name, output_format).await
        }
        CliSubcommand::Capture { capture_format } => {
            commands::capture(&mut client, &capture_format, output_format).await
        }
        CliSubcommand::Ping => commands::ping(&mut client, output_format).await,
        CliSubcommand::Version => commands::version(&mut client, output_format).await,
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
#[cfg(feature = "grpc")]
async fn run_headless_tui(addr: &str, width: u16, height: u16) -> std::io::Result<()> {
    tracing::info!("Connecting headless TUI to {addr} ({width}x{height})");

    // connect_with_size spawns the event loop automatically
    let tui = TuiAppV2Headless::connect_with_size(addr, width, height)
        .await
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::ConnectionRefused, e.to_string()))?;

    tracing::info!("Headless TUI connected and running");

    // Run until interrupted
    tokio::signal::ctrl_c().await?;
    tui.stop().await;

    Ok(())
}

/// Determine transport mode from CLI arguments.
#[allow(unused_variables)]
fn determine_transport(
    tcp: Option<u16>,
    #[cfg(feature = "grpc")] grpc: Option<u16>,
    #[cfg(unix)] socket: Option<std::path::PathBuf>,
) -> TransportMode {
    // Priority: gRPC > Unix socket > TCP > TCP fallback
    #[cfg(feature = "grpc")]
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
