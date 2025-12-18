//! Interactive REPL mode for reo-cli

use {
    crate::{
        client::{ConnectionConfig, ReoClient},
        commands,
    },
    rustyline::{DefaultEditor, error::ReadlineError},
    serde_json::json,
};

const HELP_TEXT: &str = r"Available commands:
  keys <sequence>     Inject key sequence (e.g., 'iHello<Esc>')
  mode                Get current mode
  cursor              Get cursor position
  selection           Get selection state
  screen              Get screen dimensions
  screen-content      Get rendered screen content
  buffer list         List all buffers
  buffer content      Get buffer content
  buffer open <path>  Open a file
  resize <w> <h>      Resize editor
  quit                Quit editor and exit

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

async fn handle_meta_command(line: &str, client: &mut ReoClient) -> MetaResult {
    let parts: Vec<&str> = line.splitn(2, ' ').collect();
    let cmd = parts[0];

    match cmd {
        ".help" => {
            println!("{HELP_TEXT}");
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
    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.is_empty() {
        return Ok(json!(null));
    }

    let cmd = parts[0];
    let args = &parts[1..];

    match cmd {
        "keys" => {
            if args.is_empty() {
                return Err("Usage: keys <sequence>".into());
            }
            Ok(commands::cmd_keys(client, args.join(" ").as_str()).await?)
        }
        "mode" => Ok(commands::cmd_mode(client).await?),
        "cursor" => {
            let buffer_id = parse_buffer_id_arg(args);
            Ok(commands::cmd_cursor(client, buffer_id).await?)
        }
        "selection" => {
            let buffer_id = parse_buffer_id_arg(args);
            Ok(commands::cmd_selection(client, buffer_id).await?)
        }
        "screen" => Ok(commands::cmd_screen(client).await?),
        "screen-content" => {
            let format = args.first().copied().unwrap_or("plain_text");
            Ok(commands::cmd_screen_content(client, format).await?)
        }
        "buffer" => {
            if args.is_empty() {
                return Err("Usage: buffer <list|content|open <path>>".into());
            }
            match args[0] {
                "list" => Ok(commands::cmd_buffer_list(client).await?),
                "content" => {
                    let buffer_id = parse_buffer_id_arg(&args[1..]);
                    Ok(commands::cmd_buffer_content(client, buffer_id).await?)
                }
                "open" => {
                    if args.len() < 2 {
                        return Err("Usage: buffer open <path>".into());
                    }
                    Ok(commands::cmd_buffer_open(client, args[1]).await?)
                }
                _ => Err(format!("Unknown buffer subcommand: {}", args[0]).into()),
            }
        }
        "resize" => {
            if args.len() < 2 {
                return Err("Usage: resize <width> <height>".into());
            }
            let width: u64 = args[0].parse()?;
            let height: u64 = args[1].parse()?;
            Ok(commands::cmd_resize(client, width, height).await?)
        }
        "quit" => {
            let _result = commands::cmd_quit(client).await?;
            println!("Editor quit. Exiting REPL...");
            std::process::exit(0);
        }
        _ => Err(format!("Unknown command: {cmd}. Type '.help' for available commands.").into()),
    }
}

fn parse_buffer_id_arg(args: &[&str]) -> Option<u64> {
    for (i, arg) in args.iter().enumerate() {
        if (*arg == "--buffer-id" || *arg == "-b")
            && let Some(id_str) = args.get(i + 1)
        {
            return id_str.parse().ok();
        }
    }
    None
}
