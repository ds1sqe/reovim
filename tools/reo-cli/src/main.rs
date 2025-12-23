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
mod repl;

use client::ConnectionConfig;

const DEFAULT_HOST: &str = "127.0.0.1";
const DEFAULT_PORT: u16 = 12521; // 'r'×100 + 'e'×10 + 'o' = 11400 + 1010 + 111

/// CLI client for reovim server mode
#[derive(Parser)]
#[command(name = "reo-cli")]
#[command(about = "Control reovim via JSON-RPC")]
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
    /// Inject key sequence (vim notation)
    Keys {
        /// Key sequence (e.g., "iHello<Esc>")
        keys: String,
    },

    /// Get current mode
    Mode,

    /// Get cursor position
    Cursor {
        /// Buffer ID (default: active buffer)
        #[arg(long)]
        buffer_id: Option<u64>,
    },

    /// Get selection state
    Selection {
        /// Buffer ID (default: active buffer)
        #[arg(long)]
        buffer_id: Option<u64>,
    },

    /// Get screen dimensions
    ScreenSize,

    /// Get rendered screen capture
    Capture {
        /// Output format: `plain_text`, `raw_ansi`
        #[arg(long, default_value = "plain_text")]
        format: String,
    },

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
        // Default to TCP on DEFAULT_HOST:DEFAULT_PORT
        (None, None) => Ok(ConnectionConfig::Tcp {
            host: DEFAULT_HOST.to_string(),
            port: DEFAULT_PORT,
        }),
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    // Get connection config
    let config = get_connection_config(&cli)?;

    // Interactive mode
    if cli.interactive || cli.command.is_none() {
        return repl::run_repl(&config).await;
    }

    // One-shot mode
    let mut client = client::ReoClient::connect(&config).await?;

    let command = cli.command.unwrap();

    // Handle screen-capture specially - print content directly, not as JSON
    if let Commands::Capture { ref format } = command {
        let result = commands::cmd_screen_content(&mut client, format).await?;

        // Extract and print just the content field
        if let Some(content) = result.get("content").and_then(Value::as_str) {
            println!("{content}");
        } else {
            // Fallback to JSON if content field is missing
            if cli.json {
                println!("{}", serde_json::to_string(&result)?);
            } else {
                println!("{}", serde_json::to_string_pretty(&result)?);
            }
        }
        return Ok(());
    }

    // All other commands - return JSON
    let result = match command {
        Commands::Keys { keys } => commands::cmd_keys(&mut client, &keys).await?,
        Commands::Mode => commands::cmd_mode(&mut client).await?,
        Commands::Cursor { buffer_id } => commands::cmd_cursor(&mut client, buffer_id).await?,
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
        Commands::Capture { .. } => unreachable!("handled above"),
    };

    // Output result
    if cli.json {
        println!("{}", serde_json::to_string(&result)?);
    } else {
        println!("{}", serde_json::to_string_pretty(&result)?);
    }

    Ok(())
}
