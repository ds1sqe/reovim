//! Reovim - Linux kernel-inspired text editor
//!
//! This binary provides the server entry point for the headless editor engine.
//!
//! # Server Mode
//!
//! Start the server to accept JSON-RPC connections over TCP:
//!
//! ```sh
//! # Default: TCP on 127.0.0.1:12521 (with port fallback)
//! cargo run -- --server
//!
//! # Custom TCP port
//! cargo run -- --listen-tcp 9000
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

use std::process;

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
}

fn main() {
    let args = Args::parse();

    // Determine if we should run in server mode
    let server_mode = args.server || args.listen_tcp.is_some();

    if server_mode {
        run_server(args.listen_tcp);
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
    println!("    reovim --server              # Start TCP server (default port: 12521)");
    println!("    reovim --listen-tcp 9000     # Start on custom port");
    println!();
    println!("Demo mode:");
    println!("    cargo run --example demo     # See the architecture in action");
    println!();
    println!("Use --help for more options.");
}

fn run_server(port: Option<u16>) {
    // Build config using builder pattern
    let mut config = ServerConfig::new();
    if let Some(p) = port {
        config = config.port(p);
    }

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
