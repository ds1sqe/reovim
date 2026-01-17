//! Reovim - Linux kernel-inspired text editor
//!
//! Usage:
//!   reovim server [--tcp PORT] [--socket PATH] [--stdio]
//!   reovim tui [--tcp ADDR] [--socket PATH]
//!   reovim cli [--tcp ADDR] [--socket PATH] [--repl] `<command>`
//!   reovim manager [start|stop|status]

use std::{io::BufRead, path::PathBuf, process};

use {
    clap::{Parser, Subcommand},
    runner::{
        Server, ServerConfig,
        client::{
            cli::{self, CliAction, CliArgs, OutputFormat},
            common::{ConnectionConfig, rpc::ServerMessage},
            tui::{TuiApp, TuiArgs},
        },
        manager::{ManagerClient, ManagerDaemon, is_manager_alive},
        server::SrvArgs,
    },
    serde_json::Value,
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
    /// Manage the port manager daemon.
    Manager {
        /// Manager action to perform.
        #[command(subcommand)]
        action: ManagerAction,
    },
}

/// Manager daemon actions.
#[derive(Subcommand, Debug)]
enum ManagerAction {
    /// Start the manager daemon.
    Start {
        /// Print ready signal to stdout when bound (for process coordination).
        #[arg(long, hide = true)]
        ready_signal: bool,
    },
    /// Stop the manager daemon.
    Stop,
    /// Check manager status.
    Status,
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

        Some(Command::Manager { action }) => {
            run_manager(&action);
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

/// Run integrated mode: spawn server as separate process, attach TUI.
///
/// This is the default behavior when `reovim` is invoked without arguments.
/// Similar to tmux, the server continues running after TUI exits.
///
/// # Process Architecture
///
/// ```text
/// reovim (parent)              reovim server (child)
/// └── TUI Client               └── Server process
///     └── TCP connect ←────────── READY 127.0.0.1:12521
/// ```
#[allow(unused_variables)] // files will be used later
fn run_integrated(files: &[PathBuf]) {
    use std::{
        io::BufReader,
        process::{Command, Stdio},
        time::Duration,
    };

    use runner::client::common::ConnectionConfig;

    // 1. Get current executable path
    let exe = std::env::current_exe().expect("Failed to get current executable path");

    // 2. Spawn server as child process
    let mut child = Command::new(&exe)
        .args(["server", "--tcp", "0", "--ready-signal"])
        .stdout(Stdio::piped())
        .stderr(Stdio::null()) // Suppress server logs
        .spawn()
        .expect("Failed to spawn server process");

    // 3. Read ready signal from child stdout
    let stdout = child.stdout.take().expect("Failed to get child stdout");
    let mut reader = BufReader::new(stdout);

    let addr = match read_ready_signal(&mut reader, Duration::from_secs(5)) {
        Ok(addr) => addr,
        Err(e) => {
            eprintln!("{e}");
            // Try to kill the child if it's still running
            let _ = child.kill();
            process::exit(1);
        }
    };

    // 4. Detach from child process - let it continue running independently
    // On Unix, the child becomes an orphan and gets adopted by init
    // We don't wait() on it, so it won't become a zombie
    drop(child);

    // 5. Connect TUI to the server
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("Failed to create runtime");

    rt.block_on(async {
        let config = ConnectionConfig::tcp(addr.ip().to_string(), addr.port());
        match TuiApp::connect(&config).await {
            Ok(mut app) => {
                if let Err(e) = app.run().await {
                    eprintln!("TUI error: {e}");
                    process::exit(1);
                }
                // TUI exited - server continues in background (graceful detach)
            }
            Err(e) => {
                eprintln!("Failed to connect to server at {addr}: {e}");
                process::exit(1);
            }
        }
    });
}

/// Parse ready signal from server stdout.
///
/// Reads lines until finding `READY <ip>:<port>` format.
/// Returns error if timeout or invalid format.
fn read_ready_signal<R: BufRead>(
    reader: &mut R,
    timeout: std::time::Duration,
) -> Result<std::net::SocketAddr, String> {
    use std::time::Instant;

    let start = Instant::now();
    let mut line = String::new();

    loop {
        // Check timeout
        if start.elapsed() > timeout {
            return Err("Server failed to start within 5 seconds".to_string());
        }

        line.clear();
        match reader.read_line(&mut line) {
            Ok(0) => {
                // EOF - server exited
                return Err("Server exited unexpectedly before ready signal".to_string());
            }
            Ok(_) => {
                // Try to parse ready signal
                if let Some(addr) = parse_ready_signal(&line) {
                    return Ok(addr);
                }
                // Not a ready signal, continue reading
            }
            Err(e) => {
                return Err(format!("Failed to read server output: {e}"));
            }
        }
    }
}

/// Parse a ready signal line into a socket address.
///
/// Expected format: `READY <ip>:<port>\n`
fn parse_ready_signal(line: &str) -> Option<std::net::SocketAddr> {
    line.strip_prefix("READY ")
        .and_then(|addr| addr.trim().parse().ok())
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

fn run_manager(action: &ManagerAction) {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("Failed to create runtime");

    rt.block_on(async {
        match action {
            ManagerAction::Start { ready_signal } => {
                let ready_signal = *ready_signal;
                match ManagerDaemon::bind().await {
                    Ok(daemon) => {
                        if let Err(e) = daemon.run(ready_signal).await {
                            eprintln!("Manager error: {e}");
                            process::exit(1);
                        }
                    }
                    Err(e) => {
                        eprintln!("Failed to start manager: {e}");
                        process::exit(1);
                    }
                }
            }

            ManagerAction::Stop => {
                if !is_manager_alive().await {
                    eprintln!("Manager is not running");
                    process::exit(1);
                }

                match ManagerClient::connect().await {
                    Ok(mut client) => {
                        if let Err(e) = client.shutdown().await {
                            eprintln!("Failed to stop manager: {e}");
                            process::exit(1);
                        }
                        println!("Manager stopped");
                    }
                    Err(e) => {
                        eprintln!("Failed to connect to manager: {e}");
                        process::exit(1);
                    }
                }
            }

            ManagerAction::Status => {
                if is_manager_alive().await {
                    println!("Manager is running on 127.0.0.1:{}", runner::manager::MANAGER_PORT);

                    // Try to get instance list
                    if let Ok(mut client) = ManagerClient::connect().await
                        && let Ok(instances) = client.list().await
                    {
                        if instances.is_empty() {
                            println!("No instances registered");
                        } else {
                            println!("\nRegistered instances:");
                            for info in instances {
                                println!(
                                    "  {}: {} (PID {})",
                                    info.name,
                                    info.transport.display(),
                                    info.pid
                                );
                            }
                        }
                    }
                } else {
                    println!("Manager is not running");
                }
            }
        }
    });
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

#[allow(clippy::too_many_lines)] // CLI dispatch function with many action variants
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
            use runner::manager::{ManagerClient, is_manager_alive};
            use runner::server::instance::InstanceRegistry;
            use std::collections::HashSet;

            let mut seen_addrs = HashSet::new();
            let mut found_any = false;

            // First, try to query the manager (preferred source)
            if is_manager_alive().await {
                if let Ok(mut client) = ManagerClient::connect().await {
                    if let Ok(instances) = client.list().await {
                        for instance in instances {
                            let addr = instance.transport.display();
                            seen_addrs.insert(addr.clone());
                            println!("{} ({}) pid: {}", addr, instance.name, instance.pid);
                            found_any = true;
                        }
                    }
                }
            }

            // Fall back to file registry if manager unavailable
            if !found_any {
                let registry = InstanceRegistry::new();
                if let Ok(instances) = registry.list() {
                    for instance in instances {
                        let addr = instance.transport.display();
                        seen_addrs.insert(addr.clone());
                        println!("{} ({}) pid: {}", addr, instance.name, instance.pid);
                    }
                }
            }

            // Also scan ports for any unregistered servers
            for s in discovery::list_servers() {
                let addr = format!("{}:{}", s.host, s.port);
                if !seen_addrs.contains(&addr) {
                    let pid = s.pid.map_or_else(|| "?".into(), |p| p.to_string());
                    println!("{addr} (unregistered) pid: {pid}");
                }
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
            CliAction::LogTail {
                count,
                level,
                target,
                grep,
                follow,
            } => {
                if *follow {
                    run_log_follow(
                        &mut client,
                        level.as_deref(),
                        target.as_deref(),
                        grep.as_deref(),
                        output_fmt,
                    )
                    .await
                } else {
                    cmd::cmd_log_tail(
                        &mut client,
                        *count,
                        level.as_deref(),
                        target.as_deref(),
                        grep.as_deref(),
                    )
                    .await
                }
            }
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
                    CliAction::LogTail { .. } => "log-tail",
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

/// Run log follow mode: subscribe to log notifications and stream until Ctrl+C.
async fn run_log_follow(
    client: &mut runner::client::common::rpc::RpcClient,
    level: Option<&str>,
    target: Option<&str>,
    grep: Option<&str>,
    format: OutputFormat,
) -> Result<Value, runner::client::common::rpc::RpcClientError> {
    // Subscribe with level filter
    let result: Value = cli::cmd_log_subscribe(client, level).await?;
    let sub_id = result
        .get("subscription_id")
        .and_then(Value::as_u64)
        .ok_or_else(|| {
            runner::client::common::rpc::RpcClientError::UnexpectedResponse(
                "missing subscription_id".to_string(),
            )
        })?;

    eprintln!("Following logs (Ctrl+C to stop)...");

    // Read notifications until interrupted
    let ctrl_c = tokio::signal::ctrl_c();
    tokio::pin!(ctrl_c);

    loop {
        tokio::select! {
            _ = &mut ctrl_c => {
                // Unsubscribe on Ctrl+C
                let _ = cli::cmd_log_unsubscribe(client, sub_id).await;
                eprintln!("\nStopped.");
                break;
            }
            msg = client.read_message() => {
                match msg {
                    Ok(ServerMessage::Notification(notification)) => {
                        // Only process LOG_ENTRY notifications
                        if notification.method == "LOG_ENTRY" {
                            // Apply client-side filters (target, grep)
                            if should_display_log(&notification.params, target, grep) {
                                print_log_entry(&notification.params, format);
                            }
                        }
                        // Ignore other notification types
                    }
                    Ok(ServerMessage::Response(_)) => {
                        // Ignore responses (shouldn't happen during streaming)
                    }
                    Err(e) => {
                        eprintln!("Stream error: {e}");
                        break;
                    }
                }
            }
        }
    }

    Ok(serde_json::json!({"status": "stopped"}))
}

/// Check if log entry passes client-side filters.
fn should_display_log(params: &Value, target: Option<&str>, grep: Option<&str>) -> bool {
    if let Some(target_filter) = target
        && let Some(entry_target) = params.get("target").and_then(Value::as_str)
        && !entry_target.contains(target_filter)
    {
        return false;
    }

    if let Some(grep_filter) = grep
        && let Some(message) = params.get("message").and_then(Value::as_str)
        && !message.to_lowercase().contains(&grep_filter.to_lowercase())
    {
        return false;
    }

    true
}

/// Print a log entry to stdout with color-coded level.
fn print_log_entry(params: &Value, format: OutputFormat) {
    match format {
        OutputFormat::Json => {
            println!("{}", serde_json::to_string(params).unwrap_or_default());
        }
        OutputFormat::Plain => {
            let timestamp = params
                .get("timestamp")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let level = params
                .get("level")
                .and_then(|v| v.as_str())
                .unwrap_or("INFO");
            let target = params.get("target").and_then(|v| v.as_str()).unwrap_or("");
            let message = params.get("message").and_then(|v| v.as_str()).unwrap_or("");

            // Color-coded level if stdout is a tty
            let use_color = std::io::IsTerminal::is_terminal(&std::io::stdout());
            let level_str = if use_color {
                match level.to_uppercase().as_str() {
                    "ERROR" => format!("\x1b[31m{level:5}\x1b[0m"), // red
                    "WARN" => format!("\x1b[33m{level:5}\x1b[0m"),  // yellow
                    "INFO" => format!("\x1b[32m{level:5}\x1b[0m"),  // green
                    "DEBUG" => format!("\x1b[36m{level:5}\x1b[0m"), // cyan
                    "TRACE" => format!("\x1b[90m{level:5}\x1b[0m"), // gray
                    _ => format!("{level:5}"),
                }
            } else {
                format!("{level:5}")
            };

            println!("{timestamp} {level_str} {target}: {message}");
        }
    }
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

    // Tests for parse_ready_signal

    #[test]
    fn test_parse_ready_signal_valid_localhost() {
        let addr = parse_ready_signal("READY 127.0.0.1:12521\n");
        assert!(addr.is_some());
        let addr = addr.unwrap();
        assert_eq!(addr.ip().to_string(), "127.0.0.1");
        assert_eq!(addr.port(), 12521);
    }

    #[test]
    fn test_parse_ready_signal_valid_any() {
        let addr = parse_ready_signal("READY 0.0.0.0:9000\n");
        assert!(addr.is_some());
        let addr = addr.unwrap();
        assert_eq!(addr.ip().to_string(), "0.0.0.0");
        assert_eq!(addr.port(), 9000);
    }

    #[test]
    fn test_parse_ready_signal_invalid_prefix() {
        let addr = parse_ready_signal("INVALID\n");
        assert!(addr.is_none());
    }

    #[test]
    fn test_parse_ready_signal_invalid_address() {
        let addr = parse_ready_signal("READY not_an_addr\n");
        assert!(addr.is_none());
    }

    #[test]
    fn test_parse_ready_signal_no_newline() {
        // Should still work without trailing newline
        let addr = parse_ready_signal("READY 127.0.0.1:8080");
        assert!(addr.is_some());
        assert_eq!(addr.unwrap().port(), 8080);
    }

    // Tests for read_ready_signal

    #[test]
    fn test_read_ready_signal_immediate() {
        use std::{io::Cursor, time::Duration};

        let data = "READY 127.0.0.1:12521\n";
        let mut reader = Cursor::new(data);
        let result = read_ready_signal(&mut reader, Duration::from_secs(1));
        assert!(result.is_ok());
        assert_eq!(result.unwrap().port(), 12521);
    }

    #[test]
    fn test_read_ready_signal_with_prefix_lines() {
        use std::{io::Cursor, time::Duration};

        // Server might output some lines before READY
        let data = "Loading modules...\nInitializing...\nREADY 127.0.0.1:9999\n";
        let mut reader = Cursor::new(data);
        let result = read_ready_signal(&mut reader, Duration::from_secs(1));
        assert!(result.is_ok());
        assert_eq!(result.unwrap().port(), 9999);
    }

    #[test]
    fn test_read_ready_signal_eof() {
        use std::{io::Cursor, time::Duration};

        // EOF before ready signal
        let data = "";
        let mut reader = Cursor::new(data);
        let result = read_ready_signal(&mut reader, Duration::from_secs(1));
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("exited unexpectedly"));
    }
}
