//! Reovim - Linux kernel-inspired text editor
//!
//! This binary provides the server entry point for the headless editor engine.
//!
//! # Server Mode
//!
//! Start the server to accept JSON-RPC connections:
//!
//! ```sh
//! # Default: TCP on 127.0.0.1:12521 (with port fallback)
//! cargo run -- --server
//!
//! # Custom TCP port
//! cargo run -- --listen-tcp 9000
//!
//! # Unix socket (for local IPC)
//! cargo run -- --listen-socket /tmp/reovim.sock
//!
//! # Stdio (for process embedding)
//! cargo run -- --stdio
//! ```
//!
//! # Architecture
//!
//! The server implements a tmux-style session model:
//! - Multiple clients can connect to the same session
//! - Sessions persist when clients disconnect
//! - Each session has its own editor state
//!
//! # Demo Mode
//!
//! For a complete demonstration of the kernel-driver-module architecture
//! in embedded mode, run:
//!
//! ```sh
//! cargo run --example demo
//! ```

use std::{path::PathBuf, process};

use {
    clap::Parser,
    runner::{Server, ServerConfig},
};

/// Command-line arguments for reovim.
#[derive(Parser, Debug)]
#[command(name = "reovim")]
#[command(version = "0.9.0-dev")]
#[command(about = "Linux kernel-inspired text editor", long_about = None)]
struct Args {
    /// Start in server mode (TCP on 127.0.0.1:12521 with port fallback)
    #[arg(short, long)]
    server: bool,

    /// Start server on specific TCP port
    #[arg(long, value_name = "PORT")]
    listen_tcp: Option<u16>,

    /// Start server on Unix socket
    #[cfg(unix)]
    #[arg(long, value_name = "PATH")]
    listen_socket: Option<PathBuf>,

    /// Start server in stdio mode (single client, for embedding)
    #[arg(long)]
    stdio: bool,
}

fn main() {
    let args = Args::parse();

    // Determine if we should run in server mode
    #[cfg(unix)]
    let server_mode =
        args.server || args.listen_tcp.is_some() || args.listen_socket.is_some() || args.stdio;
    #[cfg(not(unix))]
    let server_mode = args.server || args.listen_tcp.is_some() || args.stdio;

    if server_mode {
        run_server(&args);
    } else {
        print_usage();
    }
}

fn print_usage() {
    println!("reovim v0.9.0-dev - Linux kernel-inspired text editor");
    println!();
    println!("The runner crate provides mechanism (server, sessions, registries).");
    println!("Policy modules (keymap, editor, etc.) provide the actual behavior.");
    println!();
    println!("Server mode:");
    println!("    reovim --server                      # Start TCP server (default port: 12521)");
    println!("    reovim --listen-tcp 9000             # Start on custom TCP port");
    #[cfg(unix)]
    println!("    reovim --listen-socket /tmp/r.sock   # Start on Unix socket");
    println!("    reovim --stdio                       # Start in stdio mode (for embedding)");
    println!();
    println!("Demo mode:");
    println!("    cargo run --example demo             # See the architecture in action");
    println!();
    println!("Use --help for more options.");
}

// Clippy suggests map_or_else but it doesn't work well with multi-branch if-else chains
#[allow(clippy::option_if_let_else)]
fn run_server(args: &Args) {
    // Build config based on transport mode
    #[cfg(unix)]
    let config = if let Some(ref path) = args.listen_socket {
        ServerConfig::unix_socket(path)
    } else if args.stdio {
        ServerConfig::stdio()
    } else if let Some(port) = args.listen_tcp {
        ServerConfig::tcp(port)
    } else {
        ServerConfig::tcp_with_fallback()
    };

    #[cfg(not(unix))]
    let config = if args.stdio {
        ServerConfig::stdio()
    } else if let Some(port) = args.listen_tcp {
        ServerConfig::tcp(port)
    } else {
        ServerConfig::tcp_with_fallback()
    };

    // Create server
    let server = Server::new(config);

    // Run with tokio runtime
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("Failed to create tokio runtime");

    if let Err(e) = runtime.block_on(server.run()) {
        eprintln!("Server error: {e}");
        process::exit(1);
    }
}
