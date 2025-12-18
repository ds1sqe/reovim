use {
    reovim_core::{self, runtime::Runtime, screen::Screen},
    std::io::{self},
};

mod logging;

const VERSION: &str = env!("CARGO_PKG_VERSION");

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
    println!("    -h, --help             Print help information");
    println!("    -v, --version          Print version information");
    println!("    -p, --profile <NAME>   Load a configuration profile");
}

fn print_version() {
    println!("reovim {VERSION}");
}

#[tokio::main]
async fn main() -> Result<(), io::Error> {
    // Initialize logging FIRST, before any terminal manipulation
    let _log_guard = logging::init();

    tracing::info!("reovim {} starting", VERSION);

    let args: Vec<String> = std::env::args().collect();

    let mut file_path: Option<String> = None;
    let mut profile_name: Option<String> = None;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
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
            arg if arg.starts_with('-') => {
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
