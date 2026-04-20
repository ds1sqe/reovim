//! Test server harness that spawns a reovim server process.
//!
//! This is the core **mechanism** for integration testing - it handles
//! server lifecycle without any knowledge of what's being tested.
//!
//! # Log Capture (#428, #431)
//!
//! All spawned servers automatically capture stderr to per-test log files.
//! Use `spawn()` for automatic test name extraction from thread name, or
//! `spawn_with_name(name)` for explicit naming.
//!
//! # Debug Logging
//!
//! **DO NOT use `std::fs::OpenOptions` for debug logging in tests.**
//!
//! If you need debug output during test development, use the built-in log
//! capture infrastructure (`spawn()` or `spawn_with_name()`). Writing directly
//! to files with `OpenOptions` creates cleanup burdens and can leave debug
//! artifacts in the codebase.
//!
//! Server logs are automatically captured to `tmp/test-logs/{test_name}_{timestamp}.log`.

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

/// Get path to the reovim binary.
///
/// Resolution order:
/// 1. `REOVIM_TEST_BINARY` env var (explicit override)
/// 2. Inferred from `std::env::current_exe()` — the test binary lives in
///    `target/<target-dir>/debug/deps/`, so `../../reovim` gives the
///    server binary. This works with any target directory including
///    `cargo-llvm-cov`'s `target/llvm-cov-target/`.
/// 3. Fallback to `{workspace}/target/debug/reovim` via `CARGO_MANIFEST_DIR`.
#[cfg_attr(coverage_nightly, coverage(off))]
fn binary_path() -> PathBuf {
    // Check for override (useful for testing release builds)
    if let Ok(path) = std::env::var("REOVIM_TEST_BINARY") {
        return PathBuf::from(path);
    }

    // Infer from the running test binary's location.
    // Test binaries live in `target/<dir>/debug/deps/test_name-hash`.
    // The server binary is at `target/<dir>/debug/reovim`.
    if let Ok(exe) = std::env::current_exe() {
        let debug_dir = exe
            .parent() // .../debug/deps/
            .and_then(Path::parent); // .../debug/
        if let Some(dir) = debug_dir {
            let candidate = dir.join("reovim");
            if candidate.exists() {
                return candidate;
            }
        }
    }

    // Fallback: compile-time workspace root
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    PathBuf::from(manifest_dir)
        .parent()
        .expect("lib/testing should have parent")
        .parent()
        .expect("lib should have parent (workspace root)")
        .join("target/debug/reovim")
}

/// Get the target/debug directory for module loading.
///
/// Uses the same target-dir detection as `binary_path()` so modules are
/// loaded from the correct target directory (works under `cargo-llvm-cov`).
#[cfg_attr(coverage_nightly, coverage(off))]
fn workspace_module_dir() -> PathBuf {
    if let Ok(exe) = std::env::current_exe() {
        let debug_dir = exe
            .parent() // .../debug/deps/
            .and_then(Path::parent); // .../debug/
        if let Some(dir) = debug_dir
            && dir.exists()
        {
            return dir.to_path_buf();
        }
    }

    // Fallback: compile-time workspace root
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    PathBuf::from(manifest_dir)
        .parent()
        .expect("lib/testing should have parent")
        .parent()
        .expect("lib should have parent (workspace root)")
        .join("target/debug")
}

/// Read port from stderr and return the reader for continued use.
///
/// Returns the reader so logs can continue to be captured after port extraction.
#[cfg_attr(coverage_nightly, coverage(off))]
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
#[cfg_attr(coverage_nightly, coverage(off))]
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
/// All spawn methods capture server logs to `tmp/test-logs/{test_name}_{timestamp}.log`.
/// - `spawn()` - Auto-extracts test name from thread (recommended)
/// - `spawn_with_name(name)` - Explicit test name for custom naming
pub struct TestServerHarness {
    process: Child,
    port: u16,
    /// Path to the log file (if log capture is enabled).
    log_file: Option<PathBuf>,
    /// Background task capturing stderr (if log capture is enabled).
    #[allow(dead_code)]
    log_task: Option<JoinHandle<()>>,
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl TestServerHarness {
    /// Spawn server with automatic log capture.
    ///
    /// Server stderr is captured to `tmp/test-logs/{test_name}_{timestamp}.log`.
    /// Test name is auto-extracted from the current thread name (set by cargo test).
    ///
    /// For explicit test naming, use `spawn_with_name()`.
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// - Binary not found at expected path
    /// - Server fails to start within timeout
    /// - Server panics during startup
    /// - Log directory cannot be created
    pub async fn spawn() -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let test_name = std::thread::current()
            .name()
            .unwrap_or("unknown_test")
            .to_string();
        Self::spawn_inner(&test_name, &[], &[]).await
    }

    /// Spawn server with explicit test name for log capture.
    ///
    /// Server stderr is captured to `tmp/test-logs/{test_name}_{timestamp}.log`.
    /// This is invaluable for debugging test failures as it preserves the
    /// server's tracing output including mode transitions, command executions,
    /// and resolver calls.
    ///
    /// The server is started with `REOVIM_LOG=debug` by default for full
    /// tracing visibility.
    ///
    /// Use this when you need a custom test name (e.g., multi-client tests
    /// with a `_server` suffix).
    ///
    /// # Example
    ///
    /// ```ignore
    /// let harness = TestServerHarness::spawn_with_name("test_multi_client_server").await?;
    /// // ... run test ...
    /// // On failure, check: tmp/test-logs/test_multi_client_server_20260124_120000.log
    /// ```
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// - Binary not found at expected path
    /// - Server fails to start within timeout
    /// - Server panics during startup
    /// - Log directory cannot be created
    pub async fn spawn_with_name(
        test_name: &str,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        Self::spawn_inner(test_name, &[], &[]).await
    }

    /// Spawn server with extra modules loaded.
    ///
    /// Sets `REOVIM_EXTRA_MODULES` env var on the spawned server process.
    /// Test name is auto-extracted from the current thread name.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let harness = TestServerHarness::spawn_with_modules(&["textobjects"]).await?;
    /// ```
    ///
    /// # Errors
    ///
    /// Returns error if server fails to spawn or start.
    pub async fn spawn_with_modules(
        modules: &[&str],
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let test_name = std::thread::current()
            .name()
            .unwrap_or("unknown_test")
            .to_string();
        Self::spawn_inner(&test_name, modules, &[]).await
    }

    /// Spawn server with extra modules and custom environment variables.
    ///
    /// # Errors
    ///
    /// Returns error if server fails to spawn or start.
    pub async fn spawn_with_modules_and_env(
        modules: &[&str],
        env_vars: &[(&str, &str)],
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let test_name = std::thread::current()
            .name()
            .unwrap_or("unknown_test")
            .to_string();
        Self::spawn_inner(&test_name, modules, env_vars).await
    }

    /// Spawn server with custom environment variables.
    ///
    /// Passes additional env vars to the server process. Useful for
    /// overriding `XDG_DATA_HOME` to provide test fixture data.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let harness = TestServerHarness::spawn_with_env(
    ///     &[("XDG_DATA_HOME", "/tmp/test-fixtures")],
    /// ).await?;
    /// ```
    ///
    /// # Errors
    ///
    /// Returns error if server fails to spawn or start.
    pub async fn spawn_with_env(
        env_vars: &[(&str, &str)],
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let test_name = std::thread::current()
            .name()
            .unwrap_or("unknown_test")
            .to_string();
        Self::spawn_inner(&test_name, &[], env_vars).await
    }

    /// Internal spawn implementation.
    async fn spawn_inner(
        test_name: &str,
        extra_modules: &[&str],
        env_vars: &[(&str, &str)],
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

        // Use worktree modules instead of globally installed ones (#433)
        let module_dir = workspace_module_dir();

        let mut cmd = Command::new(binary_path());
        cmd.args(["server", "--grpc", "0"])
            .env("REOVIM_LOG", &log_level)
            .env("RUST_LOG", &log_level) // Enable tracing output for log capture
            .env("REOVIM_MODULE_PATH", &module_dir)
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .kill_on_drop(true);

        // Set extra modules if specified
        if !extra_modules.is_empty() {
            cmd.env("REOVIM_EXTRA_MODULES", extra_modules.join(","));
        }

        // Set custom environment variables
        for (key, val) in env_vars {
            cmd.env(key, val);
        }

        let mut process = cmd.spawn()?;

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

    /// Get the path to the log file.
    ///
    /// Returns the path to the log file where server stderr is captured.
    /// All spawn methods enable log capture, so this always returns `Some`.
    #[must_use]
    pub fn log_path(&self) -> Option<&Path> {
        self.log_file.as_deref()
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl Drop for TestServerHarness {
    fn drop(&mut self) {
        // Explicitly kill the server process to ensure cleanup.
        // This is a belt-and-suspenders approach alongside kill_on_drop(true).
        // We use start_kill() which is non-blocking, then try_wait() to reap.
        let _ = self.process.start_kill();
        let _ = self.process.try_wait();
    }
}

#[cfg(test)]
#[path = "harness_tests.rs"]
mod tests;
