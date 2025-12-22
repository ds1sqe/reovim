use {
    clap::Parser,
    reovim_core::{self, rpc::TransportConfig, runtime::Runtime, screen::Screen},
    std::io::{self, IsTerminal},
};

mod logging;
mod plugins;
mod server;

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
}

#[tokio::main]
async fn main() -> Result<(), io::Error> {
    // Initialize logging FIRST, before any terminal manipulation
    let _log_guard = logging::init();

    let cli = Cli::parse();

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

    // Normal interactive mode
    reovim_core::command::terminal::enable_raw_mode()?;
    tracing::debug!("Raw mode enabled");

    let mut screen = Screen::default();
    screen.initialize()?;
    tracing::debug!(width = screen.width(), height = screen.height(), "Screen initialized");

    let mut runtime = Runtime::with_plugins(screen, AllPlugins)
        .with_file(cli.file)
        .with_profile(cli.profile);

    // Initialize profile system (creates default profile if needed)
    runtime.init_profiles();

    runtime.init().await;

    tracing::info!("reovim shutting down");
    reovim_core::command::terminal::disable_raw_mode()
}
