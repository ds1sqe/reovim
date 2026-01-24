//! Test server harness that spawns a reovim server process.
//!
//! This is the core **mechanism** for integration testing - it handles
//! server lifecycle without any knowledge of what's being tested.
//!
//! # Log Capture (#428)
//!
//! The harness can capture server stderr to a per-test log file for debugging.
//! Use `spawn_with_log(test_name)` to enable automatic log capture.

// Test infrastructure - suppress pedantic docs requirements
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::missing_panics_doc)]

use std::{
    io::Write,
    path::{Path, PathBuf},
    process::Stdio,
    sync::atomic::{AtomicU32, Ordering},
    time::Duration,
};

use tokio::{
    io::{AsyncBufReadExt, BufReader, Lines},
    process::{Child, ChildStderr, Command},
    task::JoinHandle,
};

/// Counter for unique test identifiers
static TEST_COUNTER: AtomicU32 = AtomicU32::new(0);

/// Server startup timeout
const SERVER_STARTUP_TIMEOUT: Duration = Duration::from_secs(10);

/// Default log directory for test logs
const TEST_LOG_DIR: &str = "tmp/test-logs";

/// Get path to the reovim binary
fn binary_path() -> PathBuf {
    // Check for override (useful for testing release builds)
    if let Ok(path) = std::env::var("REOVIM_TEST_BINARY") {
        return PathBuf::from(path);
    }

    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    PathBuf::from(manifest_dir)
        .parent()
        .expect("Failed to find workspace root")
        .join("target/debug/reovim")
}

/// Read "Listening on 127.0.0.1:<port>" from stderr, consuming the reader.
///
/// This is the original function that discards stderr after port extraction.
async fn read_port_from_stderr(
    process: &mut Child,
) -> Result<u16, Box<dyn std::error::Error + Send + Sync>> {
    let stderr = process.stderr.take().ok_or("Failed to capture stderr")?;
    let mut reader = BufReader::new(stderr).lines();

    while let Some(line) = reader.next_line().await? {
        if line.starts_with("Warning:") {
            continue;
        }
        if line.contains("!!!! PANIC !!!!") {
            return Err(format!("Server panicked: {line}").into());
        }
        if let Some(rest) = line.strip_prefix("Listening on 127.0.0.1:") {
            return rest.parse::<u16>().map_err(Into::into);
        }
    }
    Err("Server exited without outputting port".into())
}

/// Read port from stderr and return the reader for continued use.
///
/// Unlike `read_port_from_stderr`, this returns the reader so logs can continue
/// to be captured after port extraction.
async fn read_port_preserving_reader(
    stderr: ChildStderr,
) -> Result<
    (u16, Lines<BufReader<ChildStderr>>, Vec<String>),
    Box<dyn std::error::Error + Send + Sync>,
> {
    let mut reader = BufReader::new(stderr).lines();
    let mut early_lines = Vec::new();

    while let Some(line) = reader.next_line().await? {
        // Save all lines for the log file
        early_lines.push(line.clone());

        if line.starts_with("Warning:") {
            continue;
        }
        if line.contains("!!!! PANIC !!!!") {
            return Err(format!("Server panicked: {line}").into());
        }
        if let Some(rest) = line.strip_prefix("Listening on 127.0.0.1:") {
            let port = rest.parse::<u16>()?;
            return Ok((port, reader, early_lines));
        }
    }
    Err("Server exited without outputting port".into())
}

/// Spawn a background task that writes stderr lines to a log file.
fn spawn_log_capture_task(
    mut reader: Lines<BufReader<ChildStderr>>,
    log_path: PathBuf,
    early_lines: Vec<String>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let Ok(mut file) = std::fs::File::create(&log_path) else {
            eprintln!("Failed to create log file: {}", log_path.display());
            return;
        };

        // Write header
        let _ = writeln!(file, "=== Server Log Started ===");

        // Write early lines (captured during port extraction)
        for line in early_lines {
            let _ = writeln!(file, "{line}");
        }

        // Continue reading and writing
        while let Ok(Some(line)) = reader.next_line().await {
            let _ = writeln!(file, "{line}");
        }

        let _ = writeln!(file, "=== Server Log Ended ===");
    })
}

/// Test harness that spawns a server process.
///
/// Automatically cleans up the server process when dropped.
///
/// # Log Capture
///
/// Use `spawn_with_log(test_name)` to automatically capture server logs
/// to `tmp/test-logs/{test_name}_{timestamp}.log`. This is invaluable for
/// debugging test failures as it preserves the server's tracing output.
pub struct TestServerHarness {
    process: Child,
    port: u16,
    /// Path to the log file (if log capture is enabled).
    log_file: Option<PathBuf>,
    /// Background task capturing stderr (if log capture is enabled).
    #[allow(dead_code)]
    log_task: Option<JoinHandle<()>>,
}

impl TestServerHarness {
    /// Spawn server on OS-assigned port (without log capture).
    ///
    /// For debugging test failures, prefer `spawn_with_log()` which captures
    /// server output to a file.
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// - Binary not found at expected path
    /// - Server fails to start within timeout
    /// - Server panics during startup
    pub async fn spawn() -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let log_level = std::env::var("REOVIM_LOG").unwrap_or_else(|_| "warn".to_string());
        let _test_id = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);

        let mut process = Command::new(binary_path())
            .args(["server", "--tcp", "0"])
            .env("REOVIM_LOG", log_level)
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()?;

        let port =
            tokio::time::timeout(SERVER_STARTUP_TIMEOUT, read_port_from_stderr(&mut process))
                .await
                .map_err(|_| "Server startup timed out (10s)")?
                .map_err(|e| format!("Failed to read port: {e}"))?;

        Ok(Self {
            process,
            port,
            log_file: None,
            log_task: None,
        })
    }

    /// Spawn server with automatic log capture to a per-test file.
    ///
    /// Server stderr is captured to `tmp/test-logs/{test_name}_{timestamp}.log`.
    /// This is invaluable for debugging test failures as it preserves the
    /// server's tracing output including mode transitions, command executions,
    /// and resolver calls.
    ///
    /// The server is started with `REOVIM_LOG=debug` by default for full
    /// tracing visibility.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let harness = TestServerHarness::spawn_with_log("test_yj_yank").await?;
    /// // ... run test ...
    /// // On failure, check: tmp/test-logs/test_yj_yank_20260124_120000.log
    /// ```
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// - Binary not found at expected path
    /// - Server fails to start within timeout
    /// - Server panics during startup
    /// - Log directory cannot be created
    pub async fn spawn_with_log(
        test_name: &str,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        // Create log directory
        let log_dir = PathBuf::from(TEST_LOG_DIR);
        std::fs::create_dir_all(&log_dir)?;

        // Generate log file path with timestamp
        let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
        let log_file = log_dir.join(format!("{test_name}_{timestamp}.log"));

        let _test_id = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);

        // Use debug level by default for test log capture
        let log_level = std::env::var("REOVIM_LOG").unwrap_or_else(|_| "debug".to_string());

        let mut process = Command::new(binary_path())
            .args(["server", "--tcp", "0"])
            .env("REOVIM_LOG", log_level)
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()?;

        // Extract stderr for port reading and log capture
        let stderr = process.stderr.take().ok_or("Failed to capture stderr")?;

        // Read port while preserving the reader for continued log capture
        let (port, reader, early_lines) =
            tokio::time::timeout(SERVER_STARTUP_TIMEOUT, read_port_preserving_reader(stderr))
                .await
                .map_err(|_| "Server startup timed out (10s)")?
                .map_err(|e| format!("Failed to read port: {e}"))?;

        // Spawn background task to continue capturing logs
        let log_task = spawn_log_capture_task(reader, log_file.clone(), early_lines);

        Ok(Self {
            process,
            port,
            log_file: Some(log_file),
            log_task: Some(log_task),
        })
    }

    /// Get the port the server is listening on.
    #[must_use]
    pub const fn port(&self) -> u16 {
        self.port
    }

    /// Get the path to the log file (if log capture is enabled).
    ///
    /// Returns `None` if the harness was created with `spawn()` instead
    /// of `spawn_with_log()`.
    #[must_use]
    pub fn log_path(&self) -> Option<&Path> {
        self.log_file.as_deref()
    }
}

impl Drop for TestServerHarness {
    fn drop(&mut self) {
        // Explicitly kill the server process to ensure cleanup.
        // This is a belt-and-suspenders approach alongside kill_on_drop(true).
        // We use start_kill() which is non-blocking, then try_wait() to reap.
        let _ = self.process.start_kill();
        let _ = self.process.try_wait();
    }
}
