//! CLI client for reovim server mode
//!
//! Connects to an existing reovim server via Unix socket or TCP
//! to control the editor programmatically.

use {
    clap::{Parser, Subcommand},
    serde_json::Value,
};

mod client;
mod commands;
mod discovery;
mod repl;

use client::ConnectionConfig;

const DEFAULT_HOST: &str = "127.0.0.1";
const DEFAULT_PORT: u16 = 12521; // 'r'×100 + 'e'×10 + 'o' = 11400 + 1010 + 111

/// CLI client for reovim server mode
#[derive(Parser)]
#[command(name = "reo-cli")]
#[command(about = "Control reovim via JSON-RPC. Shows screen/mode/cursor by default.")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Connect via Unix socket
    #[arg(long, short)]
    socket: Option<String>,

    /// Connect via TCP (host:port format, default: 127.0.0.1:12521)
    #[arg(long, short)]
    tcp: Option<String>,

    /// Run in interactive REPL mode
    #[arg(long, short)]
    interactive: bool,

    /// Output as compact JSON (for piping)
    #[arg(long)]
    json: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// List running reovim server instances
    List,

    /// Inject key sequence (vim notation)
    Keys {
        /// Key sequence (e.g., "iHello<Esc>")
        keys: String,

        /// Output format (`raw_ansi`, `plain_text`, `cell_grid`)
        #[arg(long, default_value = "raw_ansi")]
        format: String,
    },

    /// Get selection state
    Selection {
        /// Buffer ID (default: active buffer)
        #[arg(long)]
        buffer_id: Option<u64>,
    },

    /// Get screen dimensions
    ScreenSize,

    /// Buffer operations
    Buffer {
        #[command(subcommand)]
        command: BufferCommands,
    },

    /// Resize the editor
    Resize {
        /// Width in columns
        width: u64,
        /// Height in rows
        height: u64,
    },

    /// Quit the editor
    Quit,

    /// Kill the server (force shutdown, useful for CI/testing)
    Kill,

    /// Send raw JSON-RPC request
    Raw {
        /// JSON request string
        json: String,
    },
}

#[derive(Subcommand)]
enum BufferCommands {
    /// List all buffers
    List,

    /// Get buffer content
    Content {
        /// Buffer ID (default: active buffer)
        #[arg(long)]
        buffer_id: Option<u64>,
    },

    /// Set buffer content
    SetContent {
        /// New content
        content: String,
        /// Buffer ID (default: active buffer)
        #[arg(long)]
        buffer_id: Option<u64>,
    },

    /// Open a file
    Open {
        /// Path to file
        path: String,
    },
}

fn parse_tcp_address(addr: &str) -> Result<(String, u16), String> {
    let parts: Vec<&str> = addr.rsplitn(2, ':').collect();
    if parts.len() != 2 {
        return Err(format!("Invalid TCP address format: {addr}. Expected host:port"));
    }
    let port: u16 = parts[0]
        .parse()
        .map_err(|_| format!("Invalid port: {}", parts[0]))?;
    let host = parts[1].to_string();
    Ok((host, port))
}

fn get_connection_config(cli: &Cli) -> Result<ConnectionConfig, String> {
    match (&cli.socket, &cli.tcp) {
        (Some(path), None) => Ok(ConnectionConfig::UnixSocket(path.clone())),
        (None, Some(addr)) => {
            let (host, port) = parse_tcp_address(addr)?;
            Ok(ConnectionConfig::Tcp { host, port })
        }
        (Some(_), Some(_)) => Err("Cannot specify both --socket and --tcp".to_string()),
        // Auto-discover running servers
        (None, None) => {
            let servers = discovery::list_servers();
            match servers.len() {
                0 => {
                    // No servers found - fall back to default port (will fail with clear error)
                    Ok(ConnectionConfig::Tcp {
                        host: DEFAULT_HOST.to_string(),
                        port: DEFAULT_PORT,
                    })
                }
                1 => {
                    // Single server - auto-connect
                    Ok(ConnectionConfig::Tcp {
                        host: DEFAULT_HOST.to_string(),
                        port: servers[0].port,
                    })
                }
                _ => {
                    // Multiple servers - require explicit selection
                    let server_list = servers
                        .iter()
                        .map(|s| format!("  --tcp {DEFAULT_HOST}:{} (PID {})", s.port, s.pid))
                        .collect::<Vec<_>>()
                        .join("\n");
                    Err(format!("Multiple servers running. Use --tcp to specify:\n{server_list}"))
                }
            }
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    // Handle List command specially (doesn't need a connection)
    if matches!(cli.command, Some(Commands::List)) {
        let servers = discovery::list_servers();
        if servers.is_empty() {
            println!("No running reovim servers found");
        } else {
            println!("{:<10} {:<8}", "PID", "PORT");
            for server in servers {
                println!("{:<10} {:<8}", server.pid, server.port);
            }
        }
        return Ok(());
    }

    // Get connection config
    let config = get_connection_config(&cli)?;

    // Interactive mode (explicit -i flag or no command)
    if cli.interactive || cli.command.is_none() {
        return repl::run_repl(&config).await;
    }

    // Connect to server
    let mut client = client::ReoClient::connect(&config).await?;

    let command = cli.command.unwrap();

    // Keys command: inject keys then show status
    if let Commands::Keys {
        ref keys,
        ref format,
    } = command
    {
        commands::cmd_keys(&mut client, keys).await?;
        return show_status(&mut client, format).await;
    }

    // All other commands - return JSON
    let result = match command {
        Commands::Selection { buffer_id } => {
            commands::cmd_selection(&mut client, buffer_id).await?
        }
        Commands::ScreenSize => commands::cmd_screen(&mut client).await?,
        Commands::Buffer { command } => match command {
            BufferCommands::List => commands::cmd_buffer_list(&mut client).await?,
            BufferCommands::Content { buffer_id } => {
                commands::cmd_buffer_content(&mut client, buffer_id).await?
            }
            BufferCommands::SetContent { content, buffer_id } => {
                commands::cmd_buffer_set_content(&mut client, &content, buffer_id).await?
            }
            BufferCommands::Open { path } => commands::cmd_buffer_open(&mut client, &path).await?,
        },
        Commands::Resize { width, height } => {
            commands::cmd_resize(&mut client, width, height).await?
        }
        Commands::Quit => commands::cmd_quit(&mut client).await?,
        Commands::Kill => commands::cmd_kill(&mut client).await?,
        Commands::Raw { json } => commands::cmd_raw(&mut client, &json).await?,
        Commands::Keys { .. } | Commands::List => {
            unreachable!("handled above")
        }
    };

    // Output result
    if cli.json {
        println!("{}", serde_json::to_string(&result)?);
    } else {
        println!("{}", serde_json::to_string_pretty(&result)?);
    }

    Ok(())
}

/// Show default status: screen capture, mode, and cursor
async fn show_status(
    client: &mut client::ReoClient,
    format: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    // Get screen capture
    let capture = commands::cmd_screen_content(client, format).await?;
    if let Some(content) = capture.get("content").and_then(Value::as_str) {
        println!("{content}");
    }

    // Get mode and cursor
    let mode = commands::cmd_mode(client).await?;
    let cursor = commands::cmd_cursor(client, None).await?;

    // Print status line
    println!("---");
    if let Some(display) = mode.get("display").and_then(Value::as_str) {
        println!("Mode: {display}");
    }
    if let (Some(x), Some(y)) =
        (cursor.get("x").and_then(Value::as_u64), cursor.get("y").and_then(Value::as_u64))
    {
        println!("Cursor: ({x}, {y})");
    }

    Ok(())
}
