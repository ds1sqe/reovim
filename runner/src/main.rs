//! Reovim - Linux kernel-inspired text editor
//!
//! Usage:
//!   reovim server [--tcp PORT] [--socket PATH] [--stdio]
//!   reovim tui [--tcp ADDR] [--socket PATH]
//!   reovim cli [--tcp ADDR] [--socket PATH] [--repl] `<command>`

use std::{path::PathBuf, process};

use {
    clap::{Parser, Subcommand},
    runner::{
        Server, ServerConfig,
        client::{
            cli::{self, CliAction, CliArgs},
            common::ConnectionConfig,
            tui::{TuiApp, TuiArgs},
        },
        server::SrvArgs,
    },
};

/// Main CLI arguments.
#[derive(Parser, Debug)]
#[command(name = "reovim")]
#[command(version = "0.9.0-dev")]
#[command(author = "Reovim Contributors")]
#[command(about = "Linux kernel-inspired text editor")]
#[command(
    long_about = "Reovim is a Neovim-like editor with a microkernel architecture.\n\n\
    Run as a headless server, connect with TUI, or use CLI commands for automation."
)]
struct Args {
    #[command(subcommand)]
    command: Option<Command>,

    // Legacy flags (hidden for backwards compatibility)
    #[arg(short, long, hide = true)]
    server: bool,

    #[arg(long, hide = true)]
    listen_tcp: Option<u16>,

    #[cfg(unix)]
    #[arg(long, hide = true)]
    listen_socket: Option<PathBuf>,

    #[arg(long, hide = true)]
    stdio: bool,
}

/// Main command dispatcher.
#[derive(Subcommand, Debug)]
enum Command {
    /// Start the headless editor server.
    Server(SrvArgs),
    /// Connect to server with terminal UI.
    Tui(TuiArgs),
    /// Execute CLI commands.
    Cli(CliArgs),
}

fn main() {
    let args = Args::parse();

    // Handle legacy flags
    if args.command.is_none() && has_legacy_flags(&args) {
        run_server(build_legacy_server_config(&args));
        return;
    }

    match args.command {
        Some(Command::Server(srv_args)) => {
            run_server(srv_args.into_config());
        }

        Some(Command::Tui(tui_args)) => {
            let config = tui_args.into_config();
            run_tui(&config);
        }

        Some(Command::Cli(cli_args)) => {
            let config = cli_args.into_config();
            let format = cli_args.format.clone();

            if cli_args.repl || cli_args.action.is_none() {
                run_repl(&config);
            } else if let Some(ref action) = cli_args.action {
                run_cli(&config, action, &format);
            }
        }

        None => print_usage(),
    }
}

const fn has_legacy_flags(args: &Args) -> bool {
    #[cfg(unix)]
    return args.server || args.listen_tcp.is_some() || args.listen_socket.is_some() || args.stdio;
    #[cfg(not(unix))]
    return args.server || args.listen_tcp.is_some() || args.stdio;
}

#[allow(clippy::option_if_let_else)]
fn build_legacy_server_config(args: &Args) -> ServerConfig {
    #[cfg(unix)]
    if let Some(ref path) = args.listen_socket {
        return ServerConfig::unix_socket(path);
    }

    if args.stdio {
        ServerConfig::stdio()
    } else if let Some(port) = args.listen_tcp {
        ServerConfig::tcp(port)
    } else {
        ServerConfig::tcp_with_fallback()
    }
}

fn print_usage() {
    println!("reovim v0.9.0-dev - Linux kernel-inspired text editor");
    println!();
    println!("Usage: reovim <COMMAND>");
    println!();
    println!("Commands:");
    println!("    server    Start headless server");
    println!("    tui       Connect with terminal UI");
    println!("    cli       Execute commands");
    println!();
    println!("Examples:");
    println!("    reovim server");
    println!("    reovim tui");
    println!("    reovim cli keys 'iHello<Esc>'");
    println!("    reovim cli --repl");
    println!();
    println!("Use --help for options.");
}

fn run_server(config: ServerConfig) {
    let server = Server::new(config);
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("Failed to create runtime");

    if let Err(e) = rt.block_on(server.run()) {
        eprintln!("Server error: {e}");
        process::exit(1);
    }
}

fn run_tui(config: &ConnectionConfig) {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("Failed to create runtime");

    rt.block_on(async {
        match TuiApp::connect(config).await {
            Ok(mut app) => {
                if let Err(e) = app.run().await {
                    eprintln!("TUI error: {e}");
                    process::exit(1);
                }
            }
            Err(e) => {
                eprintln!("Connection failed: {e}");
                process::exit(1);
            }
        }
    });
}

fn run_repl(config: &ConnectionConfig) {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("Failed to create runtime");

    if let Err(e) = rt.block_on(cli::repl::run_repl(config)) {
        eprintln!("REPL error: {e}");
        process::exit(1);
    }
}

fn run_cli(config: &ConnectionConfig, action: &CliAction, format: &str) {
    use {
        cli::{
            commands as cmd,
            output::{OutputFormat, format_output_for_command},
        },
        runner::client::common::{RpcClient, discovery},
    };

    let output_fmt = if format == "json" {
        OutputFormat::Json
    } else {
        OutputFormat::Plain
    };

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("Failed to create runtime");

    rt.block_on(async {
        // Handle list command without connection
        if matches!(action, CliAction::List) {
            for s in discovery::list_servers() {
                let pid = s.pid.map_or_else(|| "?".into(), |p| p.to_string());
                println!("{}:{} (pid: {pid})", s.host, s.port);
            }
            return;
        }

        let mut client = match RpcClient::connect(config).await {
            Ok(c) => c,
            Err(e) => {
                eprintln!("Connection failed: {e}");
                process::exit(1);
            }
        };

        let result = match action {
            CliAction::Keys { keys } => cmd::cmd_keys(&mut client, keys).await,
            CliAction::Mode => cmd::cmd_mode(&mut client).await,
            CliAction::Cursor => cmd::cmd_cursor(&mut client).await,
            CliAction::Screen => cmd::cmd_screen(&mut client).await,
            CliAction::Content { format } => {
                cmd::cmd_screen_content(&mut client, format.as_deref().unwrap_or("plain_text"))
                    .await
            }
            CliAction::Buffers => cmd::cmd_buffer_list(&mut client).await,
            CliAction::Buffer { id } => cmd::cmd_buffer_content(&mut client, *id).await,
            CliAction::Open { path } => cmd::cmd_buffer_open(&mut client, path).await,
            CliAction::Resize { width, height } => {
                cmd::cmd_resize(&mut client, *width, *height).await
            }
            CliAction::Modules => cmd::cmd_module_list(&mut client).await,
            CliAction::Load { path } => cmd::cmd_module_load(&mut client, path).await,
            CliAction::Unload { id } => cmd::cmd_module_unload(&mut client, id).await,
            CliAction::Reload { id } => cmd::cmd_module_reload(&mut client, id).await,
            CliAction::Kill => {
                let _ = cmd::cmd_kill(&mut client).await;
                println!("Server killed");
                return;
            }
            CliAction::Raw { json } => cmd::cmd_raw(&mut client, json).await,
            CliAction::List => unreachable!(),
            CliAction::Selection => cmd::cmd_selection(&mut client, None).await,
            CliAction::Quit => cmd::cmd_quit(&mut client).await,
            CliAction::SetContent { content } => {
                cmd::cmd_buffer_set_content(&mut client, content, None).await
            }
        };

        match result {
            Ok(v) => {
                // Map action to command name for typed formatting
                let command = match action {
                    CliAction::Mode => "mode",
                    CliAction::Cursor => "cursor",
                    CliAction::Screen => "screen",
                    CliAction::Content { .. } => "content",
                    CliAction::Buffers => "buffers",
                    _ => "",
                };
                let out = format_output_for_command(&v, output_fmt, command);
                println!("{out}");
            }
            Err(e) => {
                eprintln!("Error: {e}");
                process::exit(1);
            }
        }
    });
}
