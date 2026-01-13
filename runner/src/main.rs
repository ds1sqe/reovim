use {
    clap::Parser,
    reovim_core::{
        self, command::terminal::TerminalGuard, rpc::TransportConfig, runtime::Runtime,
        screen::Screen,
    },
    std::io::{self, IsTerminal},
};

#[allow(dead_code)] // Infrastructure for kernel integration
mod buffer_manager;
#[allow(dead_code)] // Infrastructure for kernel integration
mod context;
mod dirs;
mod logging;
#[allow(unsafe_code)]
mod module;
mod plugins;
mod server;
#[allow(dead_code)] // Infrastructure for kernel integration
mod vfs;

use plugins::AllPlugins;

const DEFAULT_PORT: u16 = 12521; // 'r'×100 + 'e'×10 + 'o' = 11400 + 1010 + 111

/// A Rust-powered neovim-like text editor
#[derive(Parser)]
#[command(name = "reovim")]
#[command(version, about)]
#[allow(clippy::struct_excessive_bools)]
struct Cli {
    /// File to open
    file: Option<String>,

    /// Load a configuration profile
    #[arg(short, long)]
    profile: Option<String>,

    /// Start in server mode (TCP on 127.0.0.1:12521)
    #[arg(long)]
    server: bool,

    /// Disable terminal rendering in server mode (terminal is enabled by default)
    #[arg(long)]
    no_terminal: bool,

    /// Use stdio instead of TCP (implies --server)
    #[arg(long)]
    stdio: bool,

    /// Run for 3 minutes then exit (for testing/CI)
    #[arg(long)]
    test: bool,

    /// Listen on Unix socket (implies --server)
    #[arg(long, value_name = "PATH")]
    listen_socket: Option<String>,

    /// Listen on TCP port (default: 12521, implies --server)
    #[arg(long, value_name = "PORT")]
    listen_tcp: Option<u16>,

    /// Host for TCP (default: 127.0.0.1)
    #[arg(long, value_name = "HOST", default_value = "127.0.0.1")]
    listen_host: String,

    /// Custom log file path (default: ~/.local/share/reovim/reovim-<timestamp>.log)
    /// Special values: '-' or 'stderr' for stderr, 'none' or 'off' to disable
    #[arg(long, value_name = "PATH")]
    log: Option<String>,

    /// Enable LSP JSON-RPC message logging for debugging
    /// Values: 'default' for timestamped file, or custom path
    #[arg(long, value_name = "PATH")]
    lsp_log: Option<String>,
}

#[tokio::main]
async fn main() -> Result<(), io::Error> {
    // Parse CLI first to get log target
    let cli = Cli::parse();

    // Initialize logging BEFORE any terminal manipulation
    let log_target = logging::parse_log_target(cli.log.as_deref());
    let lsp_log_target = logging::parse_lsp_log_target(cli.lsp_log.as_deref());
    let _log_guards = logging::init_with_lsp(&log_target, &lsp_log_target);

    // Set panic hook to log panics (since terminal cleanup hides them)
    std::panic::set_hook(Box::new(|panic_info| {
        let payload = panic_info.payload();
        let msg = payload
            .downcast_ref::<&str>()
            .copied()
            .or_else(|| payload.downcast_ref::<String>().map(String::as_str))
            .unwrap_or("Unknown panic payload");

        let location = panic_info.location().map_or_else(
            || "unknown location".to_string(),
            |loc| format!("{}:{}:{}", loc.file(), loc.line(), loc.column()),
        );

        tracing::error!("!!!! PANIC !!!!");
        tracing::error!("Message: {}", msg);
        tracing::error!("Location: {}", location);
        tracing::error!("!!!! PANIC END !!!!");

        // Also try to write to stderr in case terminal is still readable
        eprintln!("\n!!!! PANIC !!!!");
        eprintln!("Message: {msg}");
        eprintln!("Location: {location}");
    }));

    tracing::info!("reovim {} starting", env!("CARGO_PKG_VERSION"));

    if let Some(ref profile) = cli.profile {
        tracing::debug!(profile = %profile, "Using profile from command line");
    }
    if let Some(ref file) = cli.file {
        tracing::debug!(file = %file, "Opening file from command line");
    }

    // Determine if server mode (explicit or implied)
    let server_mode =
        cli.server || cli.stdio || cli.listen_socket.is_some() || cli.listen_tcp.is_some();

    // Server mode: JSON-RPC over configured transport
    if server_mode {
        // Determine transport configuration (default: TCP on DEFAULT_PORT)
        let transport_config = cli.listen_socket.as_ref().map_or_else(
            || {
                if cli.stdio {
                    TransportConfig::Stdio
                } else {
                    // Default to TCP with optional custom port
                    let port = cli.listen_tcp.unwrap_or(DEFAULT_PORT);
                    TransportConfig::tcp(&cli.listen_host, port)
                }
            },
            TransportConfig::unix_socket,
        );

        // Auto-detect terminal availability: only enable if explicitly enabled AND we have a TTY
        let terminal_output = !cli.no_terminal && io::stdout().is_terminal();
        if !cli.no_terminal && !terminal_output {
            tracing::info!("Terminal output disabled: not running in a TTY");
        }
        tracing::info!(
            "Starting in server mode (dual_output={}, test_mode={}, transport={:?})",
            terminal_output,
            cli.test,
            transport_config
        );
        return server::run_server(cli.file, terminal_output, transport_config, cli.test).await;
    }

    // Normal interactive mode - guard ensures cleanup on panic/error
    tracing::info!("==> Creating terminal guard");
    let _term_guard = TerminalGuard::new_interactive()?;
    tracing::info!("==> Terminal guard created, initializing screen");

    let mut screen = Screen::default();
    screen.initialize()?;
    tracing::info!("==> Screen initialized: width={}, height={}", screen.width(), screen.height());

    tracing::info!("==> Creating runtime with plugins");
    let mut runtime = Runtime::with_plugins(screen, AllPlugins)
        .with_file(cli.file)
        .with_profile(cli.profile);

    // Initialize profile system (creates default profile if needed)
    tracing::info!("==> Initializing profile system");
    runtime.init_profiles();

    tracing::info!("==> Starting runtime event loop");
    runtime.init().await;

    tracing::info!("==> Runtime event loop exited, shutting down");
    // Guard automatically cleans up here (Drop)
    Ok(())
}
