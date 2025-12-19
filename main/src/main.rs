use {
    reovim_core::{self, rpc::TransportConfig, runtime::Runtime, screen::Screen},
    std::io::{self},
};

mod logging;
mod server;

const VERSION: &str = env!("CARGO_PKG_VERSION");
const DEFAULT_PORT: u16 = 12521; // 'r'×100 + 'e'×10 + 'o' = 11400 + 1010 + 111

fn print_help() {
    println!("reovim {VERSION}");
    println!("A Rust-powered neovim-like text editor");
    println!();
    println!("USAGE:");
    println!("    reovim [OPTIONS] [FILE]");
    println!();
    println!("ARGS:");
    println!("    [FILE]    File to open");
    println!();
    println!("OPTIONS:");
    println!("    -h, --help                  Print help information");
    println!("    -v, --version               Print version information");
    println!("    -p, --profile <NAME>        Load a configuration profile");
    println!(
        "    --server                    Start in server mode (TCP on 127.0.0.1:{DEFAULT_PORT})"
    );
    println!(
        "    --terminal                  With --server: also render to terminal (dual output)"
    );
    println!("    --stdio                     Use stdio instead of TCP (for process piping)");
    println!("    --test                      Exit when all clients disconnect (for testing/CI)");
    println!("    --listen-socket <PATH>      Listen on Unix socket");
    println!("    --listen-tcp <PORT>         Listen on TCP port (default: {DEFAULT_PORT})");
    println!("    --listen-host <HOST>        Host for TCP (default: 127.0.0.1)");
}

fn print_version() {
    println!("reovim {VERSION}");
}

#[tokio::main]
#[allow(clippy::too_many_lines)]
async fn main() -> Result<(), io::Error> {
    // Initialize logging FIRST, before any terminal manipulation
    let _log_guard = logging::init();

    tracing::info!("reovim {} starting", VERSION);

    let args: Vec<String> = std::env::args().collect();

    let mut file_path: Option<String> = None;
    let mut profile_name: Option<String> = None;
    let mut server_mode = false;
    let mut dual_output = false;
    let mut use_stdio = false;
    let mut test_mode = false;
    let mut listen_socket: Option<String> = None;
    let mut listen_tcp: Option<u16> = None;
    let mut listen_host = "127.0.0.1".to_string();

    let mut i = 1;
    while i < args.len() {
        let arg = &args[i];
        match arg.as_str() {
            "-h" | "--help" => {
                print_help();
                return Ok(());
            }
            "-v" | "--version" => {
                print_version();
                return Ok(());
            }
            "-p" | "--profile" => {
                i += 1;
                if i < args.len() {
                    profile_name = Some(args[i].clone());
                    tracing::debug!(profile = %args[i], "Using profile from command line");
                } else {
                    eprintln!("Error: --profile requires a name argument");
                    std::process::exit(1);
                }
            }
            "--server" => {
                server_mode = true;
            }
            "--terminal" => {
                dual_output = true;
            }
            "--stdio" => {
                use_stdio = true;
                server_mode = true;
            }
            "--test" => {
                test_mode = true;
            }
            "--listen-socket" => {
                i += 1;
                if i >= args.len() {
                    eprintln!("--listen-socket requires a path argument");
                    std::process::exit(1);
                }
                listen_socket = Some(args[i].clone());
                server_mode = true; // Implies server mode
            }
            "--listen-tcp" => {
                i += 1;
                if i >= args.len() {
                    eprintln!("--listen-tcp requires a port argument");
                    std::process::exit(1);
                }
                if let Ok(port) = args[i].parse::<u16>() {
                    listen_tcp = Some(port);
                } else {
                    eprintln!("Invalid port number: {}", args[i]);
                    std::process::exit(1);
                }
                server_mode = true; // Implies server mode
            }
            "--listen-host" => {
                i += 1;
                if i >= args.len() {
                    eprintln!("--listen-host requires a host argument");
                    std::process::exit(1);
                }
                listen_host.clone_from(&args[i]);
            }
            _ if arg.starts_with('-') => {
                eprintln!("Unknown option: {arg}");
                eprintln!("Use --help for usage information");
                std::process::exit(1);
            }
            arg => {
                file_path = Some(arg.to_string());
                tracing::debug!(file = %arg, "Opening file from command line");
            }
        }
        i += 1;
    }

    // Server mode: JSON-RPC over configured transport
    if server_mode {
        // Determine transport configuration (default: TCP on DEFAULT_PORT)
        let transport_config = listen_socket.map_or_else(
            || {
                if use_stdio {
                    TransportConfig::Stdio
                } else {
                    // Default to TCP with optional custom port
                    let port = listen_tcp.unwrap_or(DEFAULT_PORT);
                    TransportConfig::tcp(&listen_host, port)
                }
            },
            TransportConfig::unix_socket,
        );

        tracing::info!(
            "Starting in server mode (dual_output={}, test_mode={}, transport={:?})",
            dual_output,
            test_mode,
            transport_config
        );
        return server::run_server(file_path, dual_output, transport_config, test_mode).await;
    }

    // Normal interactive mode
    reovim_core::command::terminal::enable_raw_mode()?;
    tracing::debug!("Raw mode enabled");

    let mut screen = Screen::default();
    screen.initialize()?;
    tracing::debug!(width = screen.width(), height = screen.height(), "Screen initialized");

    let mut runtime = Runtime::new(screen)
        .with_file(file_path)
        .with_profile(profile_name);

    // Initialize profile system (creates default profile if needed)
    runtime.init_profiles();

    runtime.init().await;

    tracing::info!("reovim shutting down");
    reovim_core::command::terminal::disable_raw_mode()
}
