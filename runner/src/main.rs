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
    Run as a headless server, connect with TUI, or use CLI commands for automation.\n\n\
    Without arguments, starts server and attaches TUI (tmux-like behavior)."
)]
struct Args {
    #[command(subcommand)]
    command: Option<Command>,

    /// Start server in detached/daemon mode without TUI.
    #[arg(short = 'd', long)]
    detach: bool,

    /// Files to open on startup.
    #[arg(value_name = "FILE")]
    files: Vec<PathBuf>,

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
    /// Attach to an existing server (alias for 'tui').
    Attach(TuiArgs),
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

        Some(Command::Tui(tui_args) | Command::Attach(tui_args)) => {
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

        None => {
            if args.detach {
                // Detached mode: start server without TUI
                run_server(ServerConfig::tcp_with_fallback());
            } else {
                // Integrated mode: start server + attach TUI
                run_integrated(&args.files);
            }
        }
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

/// Run integrated mode: spawn server in background, attach TUI.
///
/// This is the default behavior when `reovim` is invoked without arguments.
/// Similar to tmux, the server continues running after TUI exits.
#[allow(unused_variables)] // files will be used in Phase 4
fn run_integrated(files: &[PathBuf]) {
    use {runner::client::common::ConnectionConfig, std::net::SocketAddr, tokio::sync::oneshot};

    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("Failed to create runtime");

    rt.block_on(async {
        // 1. Create oneshot channel for server ready signal
        let (tx, rx) = oneshot::channel::<SocketAddr>();

        // 2. Spawn server task in background
        let server = Server::new(ServerConfig::tcp_with_fallback());
        tokio::spawn(async move {
            if let Err(e) = server.run_with_ready_signal(tx).await {
                eprintln!("Server error: {e}");
            }
        });

        // 3. Wait for server to be ready and get bound address
        let Ok(addr) = rx.await else {
            eprintln!("Server failed to start");
            process::exit(1);
        };

        // 4. Connect TUI to the server
        let config = ConnectionConfig::tcp(addr.ip().to_string(), addr.port());
        match TuiApp::connect(&config).await {
            Ok(mut app) => {
                // 5. Run TUI (blocks until user exits)
                if let Err(e) = app.run().await {
                    eprintln!("TUI error: {e}");
                    process::exit(1);
                }
                // 6. TUI exited - server continues in background (graceful detach)
            }
            Err(e) => {
                eprintln!("Connection failed: {e}");
                process::exit(1);
            }
        }
    });
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
            // Debug commands
            CliAction::Version => cmd::cmd_version(&mut client).await,
            CliAction::Uptime => cmd::cmd_uptime(&mut client).await,
            CliAction::KernelState => cmd::cmd_kernel_state(&mut client).await,
            CliAction::Registers => cmd::cmd_registers(&mut client).await,
            CliAction::Marks => cmd::cmd_marks(&mut client).await,
            CliAction::ModeStack => cmd::cmd_mode_stack(&mut client).await,
            CliAction::Metrics => cmd::cmd_metrics(&mut client).await,
            CliAction::Handlers => cmd::cmd_handlers(&mut client).await,
            CliAction::LogLevel { level } => {
                cmd::cmd_log_level(&mut client, level.as_deref()).await
            }
            CliAction::LogTail { count } => cmd::cmd_log_tail(&mut client, *count).await,
            CliAction::Snapshot => cmd::cmd_snapshot(&mut client).await,
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

#[cfg(test)]
mod tests {
    use {super::*, clap::Parser};

    #[test]
    fn test_args_detach_flag_short() {
        let args = Args::try_parse_from(["reovim", "-d"]).unwrap();
        assert!(args.detach);
        assert!(args.command.is_none());
    }

    #[test]
    fn test_args_detach_flag_long() {
        let args = Args::try_parse_from(["reovim", "--detach"]).unwrap();
        assert!(args.detach);
        assert!(args.command.is_none());
    }

    #[test]
    fn test_args_files_positional() {
        let args = Args::try_parse_from(["reovim", "foo.txt", "bar.txt"]).unwrap();
        assert_eq!(args.files.len(), 2);
        assert_eq!(args.files[0].to_str().unwrap(), "foo.txt");
        assert_eq!(args.files[1].to_str().unwrap(), "bar.txt");
    }

    #[test]
    fn test_args_files_with_detach() {
        let args = Args::try_parse_from(["reovim", "-d", "foo.txt"]).unwrap();
        assert!(args.detach);
        assert_eq!(args.files.len(), 1);
    }

    #[test]
    fn test_attach_subcommand() {
        let args = Args::try_parse_from(["reovim", "attach"]).unwrap();
        assert!(matches!(args.command, Some(Command::Attach(_))));
    }

    #[test]
    fn test_attach_subcommand_with_tcp() {
        let args = Args::try_parse_from(["reovim", "attach", "--tcp", "127.0.0.1:12521"]).unwrap();
        assert!(matches!(args.command, Some(Command::Attach(_))));
    }

    #[test]
    fn test_backwards_compat_server() {
        let args = Args::try_parse_from(["reovim", "server"]).unwrap();
        assert!(matches!(args.command, Some(Command::Server(_))));
    }

    #[test]
    fn test_backwards_compat_server_with_tcp() {
        let args = Args::try_parse_from(["reovim", "server", "--tcp", "9000"]).unwrap();
        assert!(matches!(args.command, Some(Command::Server(_))));
    }

    #[test]
    fn test_backwards_compat_tui() {
        let args = Args::try_parse_from(["reovim", "tui"]).unwrap();
        assert!(matches!(args.command, Some(Command::Tui(_))));
    }

    #[test]
    fn test_backwards_compat_cli() {
        let args = Args::try_parse_from(["reovim", "cli", "mode"]).unwrap();
        assert!(matches!(args.command, Some(Command::Cli(_))));
    }

    #[test]
    fn test_no_args_has_empty_defaults() {
        let args = Args::try_parse_from(["reovim"]).unwrap();
        assert!(args.command.is_none());
        assert!(!args.detach);
        assert!(args.files.is_empty());
    }

    #[test]
    fn test_help_flag() {
        // --help should cause parse to fail with specific error
        let result = Args::try_parse_from(["reovim", "--help"]);
        assert!(result.is_err());
    }
}
