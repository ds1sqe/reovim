use super::*;

// =========================================================================
// TuiDebugConfig tests
// =========================================================================

#[test]
fn test_debug_config_new() {
    let config = TuiDebugConfig::new(PathBuf::from("/tmp/logs"), "test".to_string());
    assert_eq!(config.log_dir, PathBuf::from("/tmp/logs"));
    assert_eq!(config.name, "test");
    // start_time is YYYYMMDDHHmmss format (14 chars)
    assert_eq!(config.start_time.len(), 14);
}

#[test]
fn test_debug_config_frame_capture_path() {
    let config = TuiDebugConfig {
        log_dir: PathBuf::from("/tmp/logs"),
        name: "session1".to_string(),
        start_time: "20260101120000".to_string(),
    };
    let path = config.frame_capture_path("20260101120005");
    assert_eq!(path, PathBuf::from("/tmp/logs/frame-buffer/session1-20260101120005.frame"));
}

#[test]
fn test_debug_config_session_log_path() {
    let config = TuiDebugConfig {
        log_dir: PathBuf::from("/var/log/reovim"),
        name: "dev".to_string(),
        start_time: "20260208143000".to_string(),
    };
    let path = config.session_log_path();
    assert_eq!(path, PathBuf::from("/var/log/reovim/dev_20260208143000.log"));
}

#[test]
fn test_debug_config_clone() {
    let config = TuiDebugConfig::new(PathBuf::from("/tmp"), "clone_test".to_string());
    let cloned = config.clone();
    assert_eq!(config.log_dir, cloned.log_dir);
    assert_eq!(config.name, cloned.name);
    assert_eq!(config.start_time, cloned.start_time);
}

#[test]
fn test_debug_config_debug_impl() {
    let config = TuiDebugConfig::new(PathBuf::from("/tmp"), "dbg".to_string());
    let debug = format!("{config:?}");
    assert!(debug.contains("TuiDebugConfig"));
    assert!(debug.contains("dbg"));
}

// =========================================================================
// TuiArgs tests
// =========================================================================

#[test]
fn test_tui_args_grpc_address_default() {
    let args = TuiArgs {
        grpc_addr: None,
        headless: false,
        debug: false,
        debug_dir: None,
        debug_name: None,
        theme: None,
    };
    assert_eq!(args.grpc_address(), "127.0.0.1:50051");
}

#[test]
fn test_tui_args_grpc_address_custom() {
    let args = TuiArgs {
        grpc_addr: Some("192.168.1.100:9090".to_string()),
        headless: false,
        debug: false,
        debug_dir: None,
        debug_name: None,
        theme: None,
    };
    assert_eq!(args.grpc_address(), "192.168.1.100:9090");
}

#[test]
fn test_tui_args_debug_config_disabled() {
    let args = TuiArgs {
        grpc_addr: None,
        headless: false,
        debug: false,
        debug_dir: None,
        debug_name: None,
        theme: None,
    };
    assert!(args.into_debug_config().is_none());
}

#[test]
fn test_tui_args_debug_config_enabled() {
    let args = TuiArgs {
        grpc_addr: None,
        headless: false,
        debug: true,
        debug_dir: Some(PathBuf::from("/tmp/test-logs")),
        debug_name: Some("my-session".to_string()),
        theme: None,
    };
    let config = args.into_debug_config();
    assert!(config.is_some());
    let config = config.unwrap();
    assert_eq!(config.log_dir, PathBuf::from("/tmp/test-logs"));
    assert_eq!(config.name, "my-session");
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_tui_args_debug_config_defaults() {
    let args = TuiArgs {
        grpc_addr: None,
        headless: false,
        debug: true,
        debug_dir: None,
        debug_name: None,
        theme: None,
    };
    let config = args.into_debug_config();
    assert!(config.is_some());
    let config = config.unwrap();
    // Default name
    assert_eq!(config.name, "default");
    // Default log dir ends with reovim/logs/tui
    let log_dir = config.log_dir.to_string_lossy();
    assert!(
        log_dir.ends_with("reovim/logs/tui") || log_dir == ".",
        "Expected log_dir to end with 'reovim/logs/tui' or '.', got: {log_dir}"
    );
}

#[test]
fn test_tui_args_clone() {
    let args = TuiArgs {
        grpc_addr: Some("localhost:1234".to_string()),
        headless: true,
        debug: true,
        debug_dir: Some(PathBuf::from("/tmp")),
        debug_name: Some("test".to_string()),
        theme: Some("dark".to_string()),
    };
    let cloned = args.clone();
    assert_eq!(args.grpc_addr, cloned.grpc_addr);
    assert_eq!(args.headless, cloned.headless);
    assert_eq!(args.debug, cloned.debug);
    assert_eq!(args.debug_dir, cloned.debug_dir);
    assert_eq!(args.debug_name, cloned.debug_name);
    assert_eq!(args.theme, cloned.theme);
}
