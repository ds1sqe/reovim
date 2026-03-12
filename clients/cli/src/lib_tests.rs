use {super::*, clap::Parser};

#[test]
fn test_cli_args_parse_keys() {
    let args = CliArgs::parse_from(["reovim-cli", "keys", "--client", "1", "iHello"]);
    match &args.command {
        CliCommand::Keys { keys, client } => {
            assert_eq!(keys, "iHello");
            assert_eq!(*client, 1);
        }
        _ => panic!("Expected Keys command"),
    }
}

#[test]
fn test_cli_args_parse_mode() {
    let args = CliArgs::parse_from(["reovim-cli", "mode", "--client", "1"]);
    match &args.command {
        CliCommand::Mode { client } => {
            assert_eq!(*client, 1);
        }
        _ => panic!("Expected Mode command"),
    }
}

#[test]
fn test_cli_args_parse_cursor() {
    let args = CliArgs::parse_from(["reovim-cli", "cursor", "--client", "2"]);
    match &args.command {
        CliCommand::Cursor { client } => {
            assert_eq!(*client, 2);
        }
        _ => panic!("Expected Cursor command"),
    }
}

#[test]
fn test_cli_args_custom_address() {
    let args = CliArgs::parse_from(["reovim-cli", "--grpc", "localhost:50051", "ping"]);
    assert_eq!(args.grpc, "localhost:50051");
}

#[test]
fn test_cli_args_json_format() {
    let args = CliArgs::parse_from(["reovim-cli", "--format", "json", "version"]);
    assert_eq!(args.format, OutputFormat::Json);
}

#[test]
fn test_cli_args_clients() {
    let args = CliArgs::parse_from(["reovim-cli", "clients"]);
    assert!(matches!(args.command, CliCommand::Clients));
}

#[test]
fn test_cli_args_extension_state() {
    let args = CliArgs::parse_from(["reovim-cli", "extension-state", "whichkey", "--client", "1"]);
    match &args.command {
        CliCommand::ExtensionState { kind, client } => {
            assert_eq!(kind, "whichkey");
            assert_eq!(*client, 1);
        }
        _ => panic!("Expected ExtensionState command"),
    }
}

#[test]
fn test_cli_args_extensions() {
    let args = CliArgs::parse_from(["reovim-cli", "extensions"]);
    assert!(matches!(args.command, CliCommand::Extensions));
}

#[test]
fn test_cli_args_capture_text_format() {
    let args = CliArgs::parse_from(["reovim-cli", "capture", "--client", "1", "-f", "plain_text"]);
    match &args.command {
        CliCommand::Capture {
            client,
            capture_format,
            web_url,
            ..
        } => {
            assert_eq!(*client, Some(1));
            assert_eq!(capture_format, "plain_text");
            assert!(web_url.is_none());
        }
        _ => panic!("Expected Capture command"),
    }
}

#[test]
fn test_cli_args_capture_default_format() {
    let args = CliArgs::parse_from(["reovim-cli", "capture", "--client", "1"]);
    match &args.command {
        CliCommand::Capture { capture_format, .. } => {
            assert_eq!(capture_format, "raw_ansi");
        }
        _ => panic!("Expected Capture command"),
    }
}

#[test]
fn test_cli_args_capture_web_png() {
    let args = CliArgs::parse_from([
        "reovim-cli",
        "capture",
        "-f",
        "png",
        "--web-url",
        "http://localhost:5173",
        "--width",
        "800",
        "--height",
        "600",
        "--dpr",
        "2",
        "-o",
        "out.png",
    ]);
    match &args.command {
        CliCommand::Capture {
            client,
            capture_format,
            web_url,
            width,
            height,
            dpr,
            output,
        } => {
            assert!(client.is_none());
            assert_eq!(capture_format, "png");
            assert_eq!(web_url.as_deref(), Some("http://localhost:5173"));
            assert_eq!(*width, 800);
            assert_eq!(*height, 600);
            assert_eq!(*dpr, 2);
            assert_eq!(output.as_deref(), Some("out.png"));
        }
        _ => panic!("Expected Capture command"),
    }
}

#[test]
fn test_cli_args_capture_web_html() {
    let args = CliArgs::parse_from([
        "reovim-cli",
        "capture",
        "-f",
        "html",
        "--web-url",
        "http://localhost:5173",
    ]);
    match &args.command {
        CliCommand::Capture {
            capture_format,
            web_url,
            width,
            height,
            dpr,
            output,
            ..
        } => {
            assert_eq!(capture_format, "html");
            assert!(web_url.is_some());
            assert_eq!(*width, 1920);
            assert_eq!(*height, 1080);
            assert_eq!(*dpr, 1);
            assert!(output.is_none());
        }
        _ => panic!("Expected Capture command"),
    }
}

#[test]
fn test_cli_args_capture_no_client_no_web_url() {
    let args = CliArgs::parse_from(["reovim-cli", "capture"]);
    match &args.command {
        CliCommand::Capture {
            client, web_url, ..
        } => {
            assert!(client.is_none());
            assert!(web_url.is_none());
        }
        _ => panic!("Expected Capture command"),
    }
}
