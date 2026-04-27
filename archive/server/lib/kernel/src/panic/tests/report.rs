use {super::super::*, std::path::PathBuf};

#[test]
fn test_crash_report_summary() {
    let report = CrashReport {
        timestamp: std::time::SystemTime::now(),
        panic_message: "test panic".to_string(),
        panic_location: Some("src/main.rs:42:5".to_string()),
        backtrace: "backtrace...".to_string(),
        rust_version: "1.80.0",
        reovim_version: "0.9.0",
        thread_name: Some("main".to_string()),
        thread_id: Some(1),
        server_logs: None,
        client_dump_paths: Vec::new(),
    };

    let summary = report.summary();
    assert!(summary.contains("test panic"));
    assert!(summary.contains("src/main.rs:42:5"));
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_crash_report_write_to_file() {
    let report = CrashReport {
        timestamp: std::time::SystemTime::now(),
        panic_message: "test panic for file".to_string(),
        panic_location: Some("test.rs:1:1".to_string()),
        backtrace: "test backtrace".to_string(),
        rust_version: "1.80.0",
        reovim_version: "0.9.0-test",
        thread_name: Some("test-thread".to_string()),
        thread_id: Some(42),
        server_logs: None,
        client_dump_paths: Vec::new(),
    };

    // Try to write - may fail in CI if HOME is not set
    let result = report.write_to_file();

    // Skip test if directory is not writable (CI environment)
    if result.is_err() {
        eprintln!("Skipping test_crash_report_write_to_file: recovery dir not writable");
        return;
    }

    let path = result.unwrap();
    assert!(path.exists());

    let content = std::fs::read_to_string(&path).unwrap();
    assert!(content.contains("Reovim Crash Report"));
    assert!(content.contains("test panic for file"));
    assert!(content.contains("test backtrace"));

    // Cleanup
    std::fs::remove_file(&path).ok();
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_crash_report_with_server_logs() {
    let report = CrashReport {
        timestamp: std::time::SystemTime::now(),
        panic_message: "test panic".to_string(),
        panic_location: Some("test.rs:1:1".to_string()),
        backtrace: "backtrace".to_string(),
        rust_version: "1.80.0",
        reovim_version: "0.9.0",
        thread_name: Some("main".to_string()),
        thread_id: Some(1),
        server_logs: Some("INFO: server started\nERROR: connection lost".to_string()),
        client_dump_paths: vec![
            PathBuf::from("/tmp/client-1.log"),
            PathBuf::from("/tmp/client-2.log"),
        ],
    };

    let result = report.write_to_file();
    if result.is_err() {
        eprintln!("Skipping test: recovery dir not writable");
        return;
    }

    let path = result.unwrap();
    let content = std::fs::read_to_string(&path).unwrap();

    assert!(content.contains("Server Debug Logs:"));
    assert!(content.contains("INFO: server started"));
    assert!(content.contains("Client Debug Dumps:"));
    assert!(content.contains("/tmp/client-1.log"));
    assert!(content.contains("/tmp/client-2.log"));

    std::fs::remove_file(&path).ok();
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_crash_report_thread_info() {
    let report = CrashReport {
        timestamp: std::time::SystemTime::now(),
        panic_message: "test".to_string(),
        panic_location: None,
        backtrace: String::new(),
        rust_version: "1.80.0",
        reovim_version: "0.9.0",
        thread_name: Some("worker-42".to_string()),
        thread_id: Some(42),
        server_logs: None,
        client_dump_paths: Vec::new(),
    };

    let result = report.write_to_file();
    if result.is_err() {
        return;
    }

    let path = result.unwrap();
    let content = std::fs::read_to_string(&path).unwrap();

    assert!(content.contains("Thread: worker-42"));
    assert!(content.contains("id: 42"));

    std::fs::remove_file(&path).ok();
}

// === MC/DC: write_to_file() with server_logs present (line 146 true branch) ===

#[test]
fn test_crash_report_write_with_server_logs_covered() {
    // This test (without coverage(off)) exercises the true branch of
    // `if let Some(ref logs) = self.server_logs` (line 146) and the
    // true branch of `if !self.client_dump_paths.is_empty()` (line 154).
    let report = CrashReport {
        timestamp: std::time::SystemTime::now(),
        panic_message: "covered panic".to_string(),
        panic_location: Some("src/kernel.rs:10:1".to_string()),
        backtrace: "backtrace text".to_string(),
        rust_version: "1.92.0",
        reovim_version: "0.9.0",
        thread_name: Some("covered-thread".to_string()),
        thread_id: Some(7),
        server_logs: Some("INFO: boot complete\nWARN: low memory".to_string()),
        client_dump_paths: vec![
            PathBuf::from("/tmp/tui-dump-1.txt"),
            PathBuf::from("/tmp/tui-dump-2.txt"),
        ],
    };

    let result = report.write_to_file();
    if result.is_err() {
        // Recovery dir unavailable in this environment - skip.
        return;
    }

    let path = result.unwrap();
    let content = std::fs::read_to_string(&path).unwrap();

    // Verify server_logs branch wrote the section
    assert!(content.contains("Server Debug Logs:"));
    assert!(content.contains("INFO: boot complete"));
    assert!(content.contains("WARN: low memory"));

    // Verify client_dump_paths branch wrote the section
    assert!(content.contains("Client Debug Dumps:"));
    assert!(content.contains("/tmp/tui-dump-1.txt"));
    assert!(content.contains("/tmp/tui-dump-2.txt"));

    std::fs::remove_file(&path).ok();
}
