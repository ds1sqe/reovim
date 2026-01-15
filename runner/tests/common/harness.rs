//! Test server harness that spawns a reovim server process.
//! Ported from archive/lib/core/src/testing/server.rs

use std::{
    path::PathBuf,
    process::Stdio,
    sync::atomic::{AtomicU32, Ordering},
    time::Duration,
};

use tokio::{
    io::{AsyncBufReadExt, BufReader},
    process::{Child, Command},
};

/// Counter for unique test identifiers
static TEST_COUNTER: AtomicU32 = AtomicU32::new(0);

/// Server startup timeout
const SERVER_STARTUP_TIMEOUT: Duration = Duration::from_secs(10);

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

/// Read "Listening on 127.0.0.1:<port>" from stderr
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

/// Test harness that spawns a server process
pub struct TestServerHarness {
    process: Child,
    port: u16,
}

impl TestServerHarness {
    /// Spawn server on OS-assigned port
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

        Ok(Self { process, port })
    }

    /// Get the port
    #[must_use]
    pub const fn port(&self) -> u16 {
        self.port
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
