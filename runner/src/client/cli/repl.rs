//! Interactive REPL mode for CLI client.
//!
//! Provides a command-line interface for interactive testing.

use crate::client::common::{ConnectionConfig, RpcClient, discovery};
use crate::manager::{ManagerClient, is_manager_alive};
use crate::server::instance::InstanceRegistry;

use super::{
    commands,
    output::{OutputFormat, format_cursor, format_mode, format_output, format_screen_content},
};

/// Run interactive REPL.
///
/// # Errors
///
/// Returns error if connection fails or I/O error occurs.
#[allow(clippy::too_many_lines)]
pub async fn run_repl(config: &ConnectionConfig) -> Result<(), Box<dyn std::error::Error>> {
    let mut client = RpcClient::connect(config).await?;

    println!("reovim cli - interactive mode");
    println!("Type 'help' for commands, 'exit' to quit.\n");

    let stdin = std::io::stdin();
    let mut line = String::new();

    loop {
        // Print prompt
        eprint!("> ");

        // Read line
        line.clear();
        if stdin.read_line(&mut line)? == 0 {
            // EOF
            break;
        }

        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        // Parse command
        let args: Vec<&str> = line.split_whitespace().collect();
        let cmd = args.first().copied().unwrap_or("");

        match cmd {
            "help" | "?" => print_help(),
            "exit" | "quit" | "q" => break,

            "list" | "servers" => {
                print_server_list().await;
            }

            "connect" => {
                if args.len() < 2 {
                    eprintln!("Usage: connect <instance-name> | connect <host:port>");
                    continue;
                }
                let target = args[1];
                let new_config = if target.contains(':') {
                    // host:port format
                    match ConnectionConfig::parse_tcp(target) {
                        Ok(c) => c,
                        Err(e) => {
                            eprintln!("Invalid address: {e}");
                            continue;
                        }
                    }
                } else {
                    // Instance name
                    match ConnectionConfig::from_instance(target) {
                        Ok(c) => c,
                        Err(e) => {
                            eprintln!("Instance not found: {e}");
                            continue;
                        }
                    }
                };

                match RpcClient::connect(&new_config).await {
                    Ok(new_client) => {
                        client = new_client;
                        println!("Connected to {target}");
                    }
                    Err(e) => eprintln!("Connection failed: {e}"),
                }
            }

            "keys" | "k" => {
                if args.len() < 2 {
                    eprintln!("Usage: keys <key-sequence>");
                    continue;
                }
                let keys = args[1..].join(" ");
                match commands::cmd_keys(&mut client, &keys).await {
                    Ok(v) => println!("{}", format_output(&v, OutputFormat::Plain)),
                    Err(e) => eprintln!("Error: {e}"),
                }
            }

            "mode" | "m" => match commands::cmd_mode(&mut client).await {
                Ok(v) => println!("{}", format_mode(&v)),
                Err(e) => eprintln!("Error: {e}"),
            },

            "cursor" | "c" => match commands::cmd_cursor(&mut client).await {
                Ok(v) => println!("{}", format_cursor(&v)),
                Err(e) => eprintln!("Error: {e}"),
            },

            "screen" => match commands::cmd_screen(&mut client).await {
                Ok(v) => println!("{}", format_output(&v, OutputFormat::Plain)),
                Err(e) => eprintln!("Error: {e}"),
            },

            "content" => {
                let format = args.get(1).copied().unwrap_or("plain_text");
                match commands::cmd_screen_content(&mut client, format).await {
                    Ok(v) => println!("{}", format_screen_content(&v)),
                    Err(e) => eprintln!("Error: {e}"),
                }
            }

            "buffers" | "ls" => match commands::cmd_buffer_list(&mut client).await {
                Ok(v) => println!("{}", format_output(&v, OutputFormat::Plain)),
                Err(e) => eprintln!("Error: {e}"),
            },

            "buffer" | "b" => {
                let id = args.get(1).and_then(|s| s.parse::<u64>().ok());
                match commands::cmd_buffer_content(&mut client, id).await {
                    Ok(v) => println!("{}", format_output(&v, OutputFormat::Plain)),
                    Err(e) => eprintln!("Error: {e}"),
                }
            }

            "open" | "e" => {
                if args.len() < 2 {
                    eprintln!("Usage: open <path>");
                    continue;
                }
                match commands::cmd_buffer_open(&mut client, args[1]).await {
                    Ok(v) => println!("{}", format_output(&v, OutputFormat::Plain)),
                    Err(e) => eprintln!("Error: {e}"),
                }
            }

            "resize" => {
                if args.len() < 3 {
                    eprintln!("Usage: resize <width> <height>");
                    continue;
                }
                let width = args[1].parse::<u64>().unwrap_or(80);
                let height = args[2].parse::<u64>().unwrap_or(24);
                match commands::cmd_resize(&mut client, width, height).await {
                    Ok(_) => println!("Resized to {width}x{height}"),
                    Err(e) => eprintln!("Error: {e}"),
                }
            }

            "modules" => match commands::cmd_module_list(&mut client).await {
                Ok(v) => println!("{}", format_output(&v, OutputFormat::Plain)),
                Err(e) => eprintln!("Error: {e}"),
            },

            "load" => {
                if args.len() < 2 {
                    eprintln!("Usage: load <path>");
                    continue;
                }
                match commands::cmd_module_load(&mut client, args[1]).await {
                    Ok(v) => println!("{}", format_output(&v, OutputFormat::Plain)),
                    Err(e) => eprintln!("Error: {e}"),
                }
            }

            "unload" => {
                if args.len() < 2 {
                    eprintln!("Usage: unload <module-id>");
                    continue;
                }
                match commands::cmd_module_unload(&mut client, args[1]).await {
                    Ok(_) => println!("Unloaded {}", args[1]),
                    Err(e) => eprintln!("Error: {e}"),
                }
            }

            "reload" => {
                if args.len() < 2 {
                    eprintln!("Usage: reload <module-id>");
                    continue;
                }
                match commands::cmd_module_reload(&mut client, args[1]).await {
                    Ok(_) => println!("Reloaded {}", args[1]),
                    Err(e) => eprintln!("Error: {e}"),
                }
            }

            "kill" => match commands::cmd_kill(&mut client).await {
                Ok(_) => {
                    println!("Server killed");
                    break;
                }
                Err(e) => eprintln!("Error: {e}"),
            },

            "raw" => {
                if args.len() < 2 {
                    eprintln!("Usage: raw <json>");
                    continue;
                }
                let json_str = args[1..].join(" ");
                match commands::cmd_raw(&mut client, &json_str).await {
                    Ok(v) => println!("{}", format_output(&v, OutputFormat::Json)),
                    Err(e) => eprintln!("Error: {e}"),
                }
            }

            _ => {
                eprintln!("Unknown command: {cmd}. Type 'help' for commands.");
            }
        }
    }

    Ok(())
}

/// Print list of running servers.
async fn print_server_list() {
    use std::collections::HashSet;

    let mut seen_addrs = HashSet::new();
    let mut found_any = false;

    // First, try to query the manager
    if is_manager_alive().await {
        if let Ok(mut client) = ManagerClient::connect().await {
            if let Ok(instances) = client.list().await {
                for instance in instances {
                    let addr = instance.transport.display();
                    seen_addrs.insert(addr.clone());
                    println!("  {} ({}) pid: {}", addr, instance.name, instance.pid);
                    found_any = true;
                }
            }
        }
    }

    // Fall back to file registry
    if !found_any {
        let registry = InstanceRegistry::new();
        if let Ok(instances) = registry.list() {
            for instance in instances {
                let addr = instance.transport.display();
                seen_addrs.insert(addr.clone());
                println!("  {} ({}) pid: {}", addr, instance.name, instance.pid);
            }
        }
    }

    // Scan ports for unregistered servers
    for s in discovery::list_servers() {
        let addr = format!("{}:{}", s.host, s.port);
        if !seen_addrs.contains(&addr) {
            let pid = s.pid.map_or_else(|| "?".into(), |p| p.to_string());
            println!("  {addr} (unregistered) pid: {pid}");
        }
    }
}

/// Print help text.
fn print_help() {
    println!(
        r"Commands:
  list            List running servers
  connect <dest>  Connect to server (instance name or host:port)

  keys <seq>      Inject key sequence (e.g., keys iHello<Esc>)
  mode            Get current mode
  cursor          Get cursor position
  screen          Get screen dimensions
  content [fmt]   Get screen content (plain_text, raw_ansi, cell_grid)

  buffers         List all buffers
  buffer [id]     Get buffer content
  open <path>     Open a file

  resize <w> <h>  Resize editor

  modules         List loaded modules
  load <path>     Load a module
  unload <id>     Unload a module
  reload <id>     Reload a module

  kill            Force kill server
  raw <json>      Send raw JSON-RPC

  help            Show this help
  exit            Exit REPL

Shortcuts: k=keys, m=mode, c=cursor, b=buffer, ls=buffers, e=open, q=exit
"
    );
}
