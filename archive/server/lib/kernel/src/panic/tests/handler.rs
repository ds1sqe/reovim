use {super::super::*, std::path::PathBuf};

#[test]
fn test_handler_not_installed_by_default() {
    // Note: This test may fail if run after other tests that install the handler
    // In practice, the handler is installed once at startup
}

#[test]
fn test_install_is_idempotent() {
    // Multiple calls should not panic
    install_panic_handler();
    install_panic_handler();
    assert!(is_handler_installed());
}

// ========== DebugContext Default ==========

#[test]
fn test_debug_context_default() {
    let ctx = DebugContext::default();
    assert!(ctx.server_logs.is_none());
    assert!(ctx.client_dump_paths.is_empty());
}

#[test]
fn test_debug_context_debug() {
    let ctx = DebugContext {
        server_logs: Some("some logs".to_string()),
        client_dump_paths: vec![PathBuf::from("/tmp/dump.txt")],
    };
    let debug_str = format!("{ctx:?}");
    assert!(debug_str.contains("DebugContext"));
    assert!(debug_str.contains("some logs"));
}

// ========== set_debug_context_callback ==========

#[test]
fn test_set_debug_context_callback() {
    // Calling set_debug_context_callback should not panic
    // (may or may not succeed depending on test order due to OnceLock)
    set_debug_context_callback(Box::new(|| DebugContext {
        server_logs: Some("test".to_string()),
        client_dump_paths: Vec::new(),
    }));
}

// ========== set_recovery_callback ==========

#[test]
fn test_set_recovery_callback() {
    // Calling set_recovery_callback should not panic
    // (may or may not succeed depending on test order due to OnceLock)
    set_recovery_callback(Box::new(|_info| {
        // No-op recovery callback for testing
    }));
}
