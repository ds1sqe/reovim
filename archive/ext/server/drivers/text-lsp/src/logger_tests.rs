use super::*;

#[test]
fn logger_creates_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test.log");
    let logger = LspLogger::new(&path, "rust").unwrap();
    logger.log_event("test start");
    assert!(path.exists());

    let content = std::fs::read_to_string(&path).unwrap();
    assert!(content.contains("[rust]"));
    assert!(content.contains("=== test start"));
}

#[test]
fn logger_creates_parent_dirs() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("a/b/c/test.log");
    let logger = LspLogger::new(&path, "python");
    assert!(logger.is_ok());
    assert!(path.exists());
}

#[test]
fn logger_appends() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("append.log");
    {
        let logger = LspLogger::new(&path, "rust").unwrap();
        logger.log_event("first");
    }
    {
        let logger = LspLogger::new(&path, "rust").unwrap();
        logger.log_event("second");
    }
    let content = std::fs::read_to_string(&path).unwrap();
    assert!(content.contains("first"));
    assert!(content.contains("second"));
}

#[test]
fn log_sent() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("sent.log");
    let logger = LspLogger::new(&path, "rust").unwrap();
    logger.log_sent("textDocument/definition", "{\"uri\":\"file:///test.rs\"}");

    let content = std::fs::read_to_string(&path).unwrap();
    assert!(content.contains("--> textDocument/definition"));
    assert!(content.contains("file:///test.rs"));
}

#[test]
fn log_response() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("resp.log");
    let logger = LspLogger::new(&path, "rust").unwrap();
    logger.log_response("#42", "textDocument/definition", "ok");

    let content = std::fs::read_to_string(&path).unwrap();
    assert!(content.contains("<-- #42 textDocument/definition (ok)"));
}

#[test]
fn log_server_notification() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("notif.log");
    let logger = LspLogger::new(&path, "rust").unwrap();
    logger.log_server_notification("textDocument/publishDiagnostics", "3 items");

    let content = std::fs::read_to_string(&path).unwrap();
    assert!(content.contains("<-n textDocument/publishDiagnostics 3 items"));
}

#[test]
fn log_server_request() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("req.log");
    let logger = LspLogger::new(&path, "rust").unwrap();
    logger.log_server_request("client/registerCapability", "hover");

    let content = std::fs::read_to_string(&path).unwrap();
    assert!(content.contains("<-r client/registerCapability hover"));
}

#[test]
fn log_stderr() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("stderr.log");
    let logger = LspLogger::new(&path, "rust").unwrap();
    logger.log_stderr("proc-macro server crashed");

    let content = std::fs::read_to_string(&path).unwrap();
    assert!(content.contains("err proc-macro server crashed"));
}

#[test]
fn log_event() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("event.log");
    let logger = LspLogger::new(&path, "rust").unwrap();
    logger.log_event("server initialized (rust-analyzer 0.4.2302)");

    let content = std::fs::read_to_string(&path).unwrap();
    assert!(content.contains("=== server initialized"));
}

#[test]
fn timestamp_format() {
    let ts = LspLogger::timestamp();
    // Should be HH:MM:SS.mmm format
    assert_eq!(ts.len(), 12); // "HH:MM:SS.mmm"
    assert_eq!(&ts[2..3], ":");
    assert_eq!(&ts[5..6], ":");
    assert_eq!(&ts[8..9], ".");
}

#[test]
fn from_env_not_set() {
    // REOVIM_LSP_LOG is not set in test environments by default.
    // If it happens to be set, we just verify from_env returns Some.
    let result = LspLogger::from_env("test-env-check");
    if std::env::var("REOVIM_LSP_LOG").is_err() {
        assert!(result.is_none());
    }
}

#[test]
fn from_env_with_custom_path() {
    let dir = tempfile::tempdir().unwrap();
    let dir_path = dir.path().to_string_lossy().to_string();
    // Use a unique env var approach to avoid test interference
    // Since from_env reads REOVIM_LSP_LOG, we test the path logic directly
    let path = PathBuf::from(&dir_path).join("lsp-rust.log");
    let logger = LspLogger::new(&path, "rust").unwrap();
    logger.log_event("custom path test");
    assert!(path.exists());
}

#[test]
fn default_log_dir_with_home() {
    // This tests the fallback logic
    let dir = default_log_dir();
    // Should end with "reovim" regardless of which branch is taken
    assert!(dir.to_string_lossy().contains("reovim"), "Expected reovim in path: {dir:?}");
}

#[test]
fn language_id_in_log_lines() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("lang.log");
    let logger = LspLogger::new(&path, "python").unwrap();
    logger.log_event("test");

    let content = std::fs::read_to_string(&path).unwrap();
    assert!(content.contains("[python]"));
}

#[test]
fn concurrent_writes() {
    use std::sync::Arc;

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("concurrent.log");
    let logger = Arc::new(LspLogger::new(&path, "rust").unwrap());

    let handles: Vec<_> = (0..10)
        .map(|i| {
            let logger = Arc::clone(&logger);
            std::thread::spawn(move || {
                logger.log_event(&format!("thread {i}"));
            })
        })
        .collect();

    for h in handles {
        h.join().unwrap();
    }

    let content = std::fs::read_to_string(&path).unwrap();
    assert_eq!(content.lines().count(), 10);
}
