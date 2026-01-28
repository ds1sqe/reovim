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
