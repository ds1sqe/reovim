//! Interactive REPL mode for reo-cli

use {
    crate::{
        client::{ConnectionConfig, ReoClient},
        commands,
    },
    clap::{CommandFactory, Parser, Subcommand},
    rustyline::{DefaultEditor, error::ReadlineError},
};

/// REPL command parser
#[derive(Parser)]
#[command(name = "")]
#[command(about = "Interactive REPL for reovim")]
#[command(no_binary_name = true)]
struct ReplCli {
    #[command(subcommand)]
    command: ReplCommands,
}

#[derive(Subcommand)]
enum ReplCommands {
    /// Inject key sequence (vim notation, e.g., 'iHello<Esc>')
    Keys {
        /// Key sequence
        #[arg(trailing_var_arg = true, num_args = 1..)]
        keys: Vec<String>,
    },

    /// Get current mode
    Mode,

    /// Get cursor position
    Cursor {
        /// Buffer ID (default: active buffer)
        #[arg(long, short = 'b')]
        buffer_id: Option<u64>,
    },

    /// Get selection state
    Selection {
        /// Buffer ID (default: active buffer)
        #[arg(long, short = 'b')]
        buffer_id: Option<u64>,
    },

    /// Get screen dimensions
    ScreenSize,

    /// Get rendered screen content
    #[command(visible_alias = "capture")]
    Screen {
        /// Output format (`plain_text`, `raw_ansi`, `cell_grid`)
        format: Option<String>,
    },

    /// Buffer operations
    Buffer {
        #[command(subcommand)]
        command: ReplBufferCommands,
    },

    /// Resize the editor
    Resize {
        /// Width in columns
        width: u64,
        /// Height in rows
        height: u64,
    },

    /// Quit the editor and exit REPL
    Quit,
}

#[derive(Subcommand)]
enum ReplBufferCommands {
    /// List all buffers
    List,

    /// Get buffer content
    Content {
        /// Buffer ID (default: active buffer)
        #[arg(long, short = 'b')]
        buffer_id: Option<u64>,
    },

    /// Open a file
    Open {
        /// Path to file
        path: String,
    },
}

const META_HELP: &str = r"
Meta commands:
  .help               Show this help
  .quit / .exit       Exit REPL (keeps editor running)
  .raw <json>         Send raw JSON-RPC request
";

/// Run the interactive REPL
pub async fn run_repl(config: &ConnectionConfig) -> Result<(), Box<dyn std::error::Error>> {
    println!("reo-cli interactive mode");
    println!("Type '.help' for available commands, '.quit' to exit\n");

    let mut client = ReoClient::connect(config).await?;
    let mut rl = DefaultEditor::new()?;

    loop {
        // Use block_in_place for synchronous readline
        let readline = tokio::task::block_in_place(|| rl.readline("reo> "));

        match readline {
            Ok(line) => {
                let line = line.trim();
                if line.is_empty() {
                    continue;
                }

                let _ = rl.add_history_entry(line);

                // Handle meta commands
                if line.starts_with('.') {
                    match handle_meta_command(line, &mut client).await {
                        MetaResult::Continue => continue,
                        MetaResult::Exit => break,
                        MetaResult::Error(e) => {
                            eprintln!("Error: {e}");
                            continue;
                        }
                    }
                }

                // Handle regular commands
                match execute_command(line, &mut client).await {
                    Ok(result) => {
                        // Pretty print result
                        if let Ok(pretty) = serde_json::to_string_pretty(&result) {
                            println!("{pretty}");
                        } else {
                            println!("{result}");
                        }
                    }
                    Err(e) => eprintln!("Error: {e}"),
                }
            }
            Err(ReadlineError::Interrupted | ReadlineError::Eof) => {
                println!("Exiting...");
                break;
            }
            Err(err) => {
                eprintln!("Error: {err}");
                break;
            }
        }
    }

    Ok(())
}

enum MetaResult {
    Continue,
    Exit,
    Error(String),
}

fn print_help() {
    let mut cmd = ReplCli::command();
    let help = cmd.render_help();
    println!("{help}");
    println!("{META_HELP}");
}

async fn handle_meta_command(line: &str, client: &mut ReoClient) -> MetaResult {
    let parts: Vec<&str> = line.splitn(2, ' ').collect();
    let cmd = parts[0];

    match cmd {
        ".help" => {
            print_help();
            MetaResult::Continue
        }
        ".quit" | ".exit" => MetaResult::Exit,
        ".raw" => {
            if parts.len() < 2 {
                return MetaResult::Error("Usage: .raw <json>".to_string());
            }
            match commands::cmd_raw(client, parts[1]).await {
                Ok(result) => {
                    if let Ok(pretty) = serde_json::to_string_pretty(&result) {
                        println!("{pretty}");
                    }
                    MetaResult::Continue
                }
                Err(e) => MetaResult::Error(e.to_string()),
            }
        }
        _ => MetaResult::Error(format!("Unknown meta command: {cmd}")),
    }
}

async fn execute_command(
    line: &str,
    client: &mut ReoClient,
) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    // Parse using clap
    let args: Vec<&str> = line.split_whitespace().collect();
    if args.is_empty() {
        return Ok(serde_json::json!(null));
    }

    let cli = match ReplCli::try_parse_from(args) {
        Ok(cli) => cli,
        Err(e) => {
            // Print clap's error message (includes help for that command)
            return Err(e.to_string().into());
        }
    };

    match cli.command {
        ReplCommands::Keys { keys } => {
            let key_str = keys.join(" ");
            Ok(commands::cmd_keys(client, &key_str).await?)
        }
        ReplCommands::Mode => Ok(commands::cmd_mode(client).await?),
        ReplCommands::Cursor { buffer_id } => Ok(commands::cmd_cursor(client, buffer_id).await?),
        ReplCommands::Selection { buffer_id } => {
            Ok(commands::cmd_selection(client, buffer_id).await?)
        }
        ReplCommands::ScreenSize => Ok(commands::cmd_screen(client).await?),
        ReplCommands::Screen { format } => {
            let fmt = format.as_deref().unwrap_or("plain_text");
            Ok(commands::cmd_screen_content(client, fmt).await?)
        }
        ReplCommands::Buffer { command } => match command {
            ReplBufferCommands::List => Ok(commands::cmd_buffer_list(client).await?),
            ReplBufferCommands::Content { buffer_id } => {
                Ok(commands::cmd_buffer_content(client, buffer_id).await?)
            }
            ReplBufferCommands::Open { path } => {
                Ok(commands::cmd_buffer_open(client, &path).await?)
            }
        },
        ReplCommands::Resize { width, height } => {
            Ok(commands::cmd_resize(client, width, height).await?)
        }
        ReplCommands::Quit => {
            let _result = commands::cmd_quit(client).await?;
            println!("Editor quit. Exiting REPL...");
            std::process::exit(0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_help_generation() {
        // Verify that help can be rendered without panicking
        let mut cmd = ReplCli::command();
        let help = cmd.render_help().to_string();

        // Check that key commands are present
        assert!(help.contains("keys"), "Help should contain 'keys' command");
        assert!(help.contains("mode"), "Help should contain 'mode' command");
        assert!(help.contains("cursor"), "Help should contain 'cursor' command");
        assert!(help.contains("selection"), "Help should contain 'selection' command");
        assert!(help.contains("screen-size"), "Help should contain 'screen-size' command");
        assert!(help.contains("screen"), "Help should contain 'screen' command");
        assert!(help.contains("capture"), "Help should contain 'capture' alias");
        assert!(help.contains("buffer"), "Help should contain 'buffer' command");
        assert!(help.contains("resize"), "Help should contain 'resize' command");
        assert!(help.contains("quit"), "Help should contain 'quit' command");
    }

    #[test]
    fn test_command_parsing_keys() {
        let cli = ReplCli::try_parse_from(["keys", "iHello<Esc>"]).unwrap();
        match cli.command {
            ReplCommands::Keys { keys } => {
                assert_eq!(keys, vec!["iHello<Esc>"]);
            }
            _ => panic!("Expected Keys command"),
        }
    }

    #[test]
    fn test_command_parsing_keys_with_spaces() {
        let cli = ReplCli::try_parse_from(["keys", "i", "Hello", "World"]).unwrap();
        match cli.command {
            ReplCommands::Keys { keys } => {
                assert_eq!(keys, vec!["i", "Hello", "World"]);
            }
            _ => panic!("Expected Keys command"),
        }
    }

    #[test]
    fn test_command_parsing_mode() {
        let cli = ReplCli::try_parse_from(["mode"]).unwrap();
        assert!(matches!(cli.command, ReplCommands::Mode));
    }

    #[test]
    fn test_command_parsing_cursor_with_buffer_id() {
        let cli = ReplCli::try_parse_from(["cursor", "-b", "42"]).unwrap();
        match cli.command {
            ReplCommands::Cursor { buffer_id } => {
                assert_eq!(buffer_id, Some(42));
            }
            _ => panic!("Expected Cursor command"),
        }
    }

    #[test]
    fn test_command_parsing_screen_size() {
        let cli = ReplCli::try_parse_from(["screen-size"]).unwrap();
        assert!(matches!(cli.command, ReplCommands::ScreenSize));
    }

    #[test]
    fn test_command_parsing_screen_with_format() {
        let cli = ReplCli::try_parse_from(["screen", "raw_ansi"]).unwrap();
        match cli.command {
            ReplCommands::Screen { format } => {
                assert_eq!(format, Some("raw_ansi".to_string()));
            }
            _ => panic!("Expected Screen command"),
        }
    }

    #[test]
    fn test_command_parsing_capture_alias() {
        let cli = ReplCli::try_parse_from(["capture"]).unwrap();
        assert!(matches!(cli.command, ReplCommands::Screen { .. }));
    }

    #[test]
    fn test_command_parsing_buffer_list() {
        let cli = ReplCli::try_parse_from(["buffer", "list"]).unwrap();
        match cli.command {
            ReplCommands::Buffer { command } => {
                assert!(matches!(command, ReplBufferCommands::List));
            }
            _ => panic!("Expected Buffer command"),
        }
    }

    #[test]
    fn test_command_parsing_resize() {
        let cli = ReplCli::try_parse_from(["resize", "80", "24"]).unwrap();
        match cli.command {
            ReplCommands::Resize { width, height } => {
                assert_eq!(width, 80);
                assert_eq!(height, 24);
            }
            _ => panic!("Expected Resize command"),
        }
    }
}
