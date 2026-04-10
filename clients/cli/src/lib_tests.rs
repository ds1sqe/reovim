use {
    super::*,
    clap::Parser,
    reovim_driver_module_registry::{InstalledModule, InstalledModules, ModuleSource},
    std::{
        ffi::OsString,
        sync::{Mutex, MutexGuard, OnceLock},
    },
};

fn env_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

struct XdgDataHomeGuard {
    _lock: MutexGuard<'static, ()>,
    old: Option<OsString>,
}

impl Drop for XdgDataHomeGuard {
    #[allow(unsafe_code)]
    fn drop(&mut self) {
        unsafe {
            if let Some(value) = self.old.take() {
                std::env::set_var("XDG_DATA_HOME", value);
            } else {
                std::env::remove_var("XDG_DATA_HOME");
            }
        }
    }
}

#[allow(unsafe_code)]
fn set_xdg_data_home(path: &std::path::Path) -> XdgDataHomeGuard {
    let lock = env_lock().lock().expect("env lock should not be poisoned");
    let old = std::env::var_os("XDG_DATA_HOME");
    unsafe {
        std::env::set_var("XDG_DATA_HOME", path);
    }
    XdgDataHomeGuard { _lock: lock, old }
}

fn save_installed_module(root: &std::path::Path, module: InstalledModule) {
    let modules_dir = root.join("reovim/modules");
    std::fs::create_dir_all(&modules_dir).expect("modules dir should exist");
    let mut installed = InstalledModules::new();
    installed.insert(module);
    installed
        .save(&modules_dir.join("installed.json"))
        .expect("installed.json should save");
}

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

#[test]
fn test_cli_args_parse_module_install_git() {
    let args = CliArgs::parse_from([
        "reovim-cli",
        "module",
        "install",
        "https://github.com/user/repo.git",
    ]);
    match &args.command {
        CliCommand::Module {
            subcommand: ModuleSubcommand::Install { source, rev },
        } => {
            assert_eq!(source, "https://github.com/user/repo.git");
            assert!(rev.is_none());
        }
        _ => panic!("Expected module install command"),
    }
}

#[test]
fn test_cli_args_parse_module_install_local_with_rev() {
    let args = CliArgs::parse_from([
        "reovim-cli",
        "module",
        "install",
        "./my-module",
        "--rev",
        "v1.0.0",
    ]);
    match &args.command {
        CliCommand::Module {
            subcommand: ModuleSubcommand::Install { source, rev },
        } => {
            assert_eq!(source, "./my-module");
            assert_eq!(rev.as_deref(), Some("v1.0.0"));
        }
        _ => panic!("Expected module install command"),
    }
}

#[test]
fn test_cli_args_parse_module_install_git_with_rev() {
    let args = CliArgs::parse_from([
        "reovim-cli",
        "module",
        "install",
        "https://github.com/user/repo.git",
        "--rev",
        "v1.0.0",
    ]);
    match &args.command {
        CliCommand::Module {
            subcommand: ModuleSubcommand::Install { source, rev },
        } => {
            assert_eq!(source, "https://github.com/user/repo.git");
            assert_eq!(rev.as_deref(), Some("v1.0.0"));
        }
        _ => panic!("Expected module install command"),
    }
}

#[test]
fn test_cli_args_parse_module_install_local_without_rev() {
    let args = CliArgs::parse_from(["reovim-cli", "module", "install", "./my-module"]);
    match &args.command {
        CliCommand::Module {
            subcommand: ModuleSubcommand::Install { source, rev },
        } => {
            assert_eq!(source, "./my-module");
            assert!(rev.is_none());
        }
        _ => panic!("Expected module install command"),
    }
}

#[test]
fn test_cli_args_parse_module_remove() {
    let args = CliArgs::parse_from(["reovim-cli", "module", "remove", "sample"]);
    match &args.command {
        CliCommand::Module {
            subcommand: ModuleSubcommand::Remove { id },
        } => assert_eq!(id, "sample"),
        _ => panic!("Expected module remove command"),
    }
}

#[test]
fn test_cli_args_parse_module_update_one() {
    let args = CliArgs::parse_from(["reovim-cli", "module", "update", "sample"]);
    match &args.command {
        CliCommand::Module {
            subcommand: ModuleSubcommand::Update { id },
        } => assert_eq!(id.as_deref(), Some("sample")),
        _ => panic!("Expected module update command"),
    }
}

#[test]
fn test_cli_args_parse_module_update_all() {
    let args = CliArgs::parse_from(["reovim-cli", "module", "update"]);
    match &args.command {
        CliCommand::Module {
            subcommand: ModuleSubcommand::Update { id },
        } => assert!(id.is_none()),
        _ => panic!("Expected module update command"),
    }
}

#[test]
fn test_cli_args_parse_module_list_loaded_with_global_grpc() {
    let args = CliArgs::parse_from([
        "reovim-cli",
        "--grpc",
        "127.0.0.1:12540",
        "module",
        "list",
        "--loaded",
    ]);
    assert_eq!(args.grpc, "127.0.0.1:12540");
    match &args.command {
        CliCommand::Module {
            subcommand: ModuleSubcommand::List { loaded },
        } => assert!(*loaded),
        _ => panic!("Expected module list command"),
    }
}

#[test]
fn test_cli_args_parse_module_list() {
    let args = CliArgs::parse_from(["reovim-cli", "module", "list"]);
    match &args.command {
        CliCommand::Module {
            subcommand: ModuleSubcommand::List { loaded },
        } => assert!(!loaded),
        _ => panic!("Expected module list command"),
    }
}

#[test]
fn test_cli_args_parse_module_info() {
    let args = CliArgs::parse_from(["reovim-cli", "module", "info", "sample"]);
    match &args.command {
        CliCommand::Module {
            subcommand: ModuleSubcommand::Info { id },
        } => assert_eq!(id, "sample"),
        _ => panic!("Expected module info command"),
    }
}

#[test]
fn test_cli_args_parse_module_check() {
    let args = CliArgs::parse_from(["reovim-cli", "module", "check"]);
    assert!(matches!(
        args.command,
        CliCommand::Module {
            subcommand: ModuleSubcommand::Check
        }
    ));
}

#[tokio::test(flavor = "current_thread")]
async fn test_execute_module_list_does_not_require_running_server() {
    let temp = tempfile::tempdir().expect("tempdir should create");
    let _guard = set_xdg_data_home(temp.path());
    let args = CliArgs::parse_from(["reovim-cli", "module", "list"]);

    let output = args
        .execute()
        .await
        .expect("module list should not require gRPC server");

    assert!(output.contains("No modules installed"));
}

#[tokio::test(flavor = "current_thread")]
async fn test_execute_module_check_does_not_require_running_server() {
    let temp = tempfile::tempdir().expect("tempdir should create");
    let _guard = set_xdg_data_home(temp.path());
    let args = CliArgs::parse_from(["reovim-cli", "module", "check"]);

    let output = args
        .execute()
        .await
        .expect("module check should not require gRPC server");

    assert!(output.contains("OK"));
}

#[tokio::test(flavor = "current_thread")]
async fn test_execute_module_info_does_not_require_running_server() {
    let temp = tempfile::tempdir().expect("tempdir should create");
    let library_path = temp.path().join("reovim/modules/sample/libsample.so");
    std::fs::create_dir_all(
        library_path
            .parent()
            .expect("library path should have parent directory"),
    )
    .expect("library dir should exist");
    std::fs::write(&library_path, b"fake").expect("library placeholder should write");
    save_installed_module(
        temp.path(),
        InstalledModule {
            id: "sample".to_string(),
            version: "1.0.0".to_string(),
            source: ModuleSource::path(temp.path().join("sample").display().to_string()),
            install_path: temp.path().join("sample"),
            library_path: Some(library_path),
        },
    );
    let _guard = set_xdg_data_home(temp.path());
    let args = CliArgs::parse_from(["reovim-cli", "module", "info", "sample"]);

    let output = args
        .execute()
        .await
        .expect("module info should not require gRPC server");

    assert!(output.contains("ID:        sample"));
}

#[tokio::test(flavor = "current_thread")]
async fn test_execute_module_remove_failure_is_local_not_connection() {
    let temp = tempfile::tempdir().expect("tempdir should create");
    let _guard = set_xdg_data_home(temp.path());
    let args = CliArgs::parse_from(["reovim-cli", "module", "remove", "missing"]);

    let err = args
        .execute()
        .await
        .expect_err("missing local remove should fail without gRPC");

    assert!(matches!(err, GrpcClientError::OperationFailed(_)));
}

#[tokio::test(flavor = "current_thread")]
async fn test_execute_module_update_failure_is_local_not_connection() {
    let temp = tempfile::tempdir().expect("tempdir should create");
    let _guard = set_xdg_data_home(temp.path());
    let args = CliArgs::parse_from(["reovim-cli", "module", "update", "missing"]);

    let err = args
        .execute()
        .await
        .expect_err("missing local update should fail without gRPC");

    assert!(matches!(err, GrpcClientError::OperationFailed(_)));
}

#[tokio::test(flavor = "current_thread")]
async fn test_execute_module_list_loaded_returns_connection_error_when_unreachable() {
    let args = CliArgs::parse_from([
        "reovim-cli",
        "--grpc",
        "127.0.0.1:9",
        "module",
        "list",
        "--loaded",
    ]);

    let err = args
        .execute()
        .await
        .expect_err("loaded module list should fail when server is unreachable");

    assert!(matches!(err, GrpcClientError::ConnectionFailed(_)));
}
