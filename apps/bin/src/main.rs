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
//! # Start with gRPC transport
//! reovim-new server --grpc 12540
//!
//! # Start with Unix socket (Unix only)
//! reovim-new server --socket /tmp/reovim.sock
//!
//! # Connect interactive TUI to running server
//! reovim-new tui --grpc 127.0.0.1:12540
//!
//! # Connect headless TUI (for scripting/testing)
//! reovim-new tui --grpc 127.0.0.1:12540 --headless
//! ```

mod bootstrap;

use {
    clap::{Parser, Subcommand},
    reovim_server::{Server, ServerConfig, TransportMode},
};

use {
    reovim_client_cli::OutputFormat,
    reovim_client_tui::{TuiAppV2, TuiAppV2Headless},
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
    /// Presence operations for multi-client awareness.
    Presence {
        #[command(subcommand)]
        action: PresenceAction,
    },
}

/// Presence subcommands.
#[derive(Debug, Subcommand)]
enum PresenceAction {
    /// Join the session with a display name.
    Join {
        /// Display name for this client.
        name: String,
        /// Client type identifier.
        #[arg(long, default_value = "cli")]
        client_type: String,
    },
    /// Leave the session.
    Leave {
        /// Client ID to remove.
        client_id: u64,
    },
    /// List all connected clients.
    List,
    /// Update presence state.
    Update {
        /// Client ID making the update.
        client_id: u64,
        /// Buffer ID to switch to.
        #[arg(long)]
        buffer: Option<u64>,
        /// Cursor line position.
        #[arg(long)]
        line: Option<u64>,
        /// Cursor column position.
        #[arg(long)]
        column: Option<u64>,
        /// Mode name.
        #[arg(long)]
        mode: Option<String>,
    },
    /// Set sync mode to follow another client.
    Follow {
        /// Client ID setting the mode.
        client_id: u64,
        /// Target client ID to follow.
        target: u64,
    },
    /// Set sync mode to present (others can follow you).
    Present {
        /// Client ID to set as presenter.
        client_id: u64,
    },
    /// Set sync mode to independent.
    Independent {
        /// Client ID to set as independent.
        client_id: u64,
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

            // Create server with module-initialized session factory
            let server =
                Server::with_session_factory(config, Box::new(bootstrap::create_session_state));
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
        CliSubcommand::Presence { action } => match action {
            PresenceAction::Join { name, client_type } => {
                commands::presence_join(&mut client, &client_type, &name, output_format).await
            }
            PresenceAction::Leave { client_id } => {
                commands::presence_leave(&mut client, client_id, output_format).await
            }
            PresenceAction::List => commands::presence_list(&mut client, output_format).await,
            PresenceAction::Update {
                client_id,
                buffer,
                line,
                column,
                mode,
            } => {
                commands::presence_update(
                    &mut client,
                    client_id,
                    buffer,
                    line,
                    column,
                    mode,
                    output_format,
                )
                .await
            }
            PresenceAction::Follow { client_id, target } => {
                commands::presence_set_sync_mode(
                    &mut client,
                    client_id,
                    1,
                    Some(target),
                    output_format,
                )
                .await
            }
            PresenceAction::Present { client_id } => {
                commands::presence_set_sync_mode(&mut client, client_id, 2, None, output_format)
                    .await
            }
            PresenceAction::Independent { client_id } => {
                commands::presence_set_sync_mode(&mut client, client_id, 0, None, output_format)
                    .await
            }
        },
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

/// Run interactive TUI.
async fn run_interactive_tui(addr: &str) -> std::io::Result<()> {
    tracing::info!("Connecting interactive TUI to {addr}");

    let mut tui = TuiAppV2::connect(addr, None, None)
        .await
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::ConnectionRefused, e.to_string()))?;

    let result = tui
        .run()
        .await
        .map_err(|e| std::io::Error::other(e.to_string()));

    drop(tui);
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
