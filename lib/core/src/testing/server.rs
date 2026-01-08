//! Server test harness for integration testing
//!
//! Spawns a reovim server process and provides a client for testing.
//! Uses OS-assigned ports (port 0) to avoid port contention when running
//! tests in parallel across multiple processes.

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

use super::client::TestClient;

/// Counter for unique test identifiers (combined with PID for uniqueness across processes)
static TEST_COUNTER: AtomicU32 = AtomicU32::new(0);

/// Get the path to the reovim binary
fn binary_path() -> PathBuf {
    // CARGO_MANIFEST_DIR is lib/core, so we go up two levels to workspace root
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    PathBuf::from(manifest_dir)
        .parent()
        .and_then(|p| p.parent())
        .expect("Failed to find workspace root")
        .join("target/debug/reovim")
}

/// Read the actual port from server's stderr output
///
/// The server prints "Listening on 127.0.0.1:<port>" when ready.
/// This function handles potential early output (warnings, panics).
async fn read_port_from_stderr(
    process: &mut Child,
) -> Result<u16, Box<dyn std::error::Error + Send + Sync>> {
    let stderr = process.stderr.take().ok_or("Failed to capture stderr")?;

    let mut reader = BufReader::new(stderr).lines();

    while let Some(line) = reader.next_line().await? {
        // Skip warning messages from logging setup
        if line.starts_with("Warning:") {
            continue;
        }

        // Detect panic - server won't start
        if line.contains("!!!! PANIC !!!!") {
            return Err(format!("Server panicked during startup: {line}").into());
        }

        // Parse port from "Listening on 127.0.0.1:<port>"
        if let Some(rest) = line.strip_prefix("Listening on 127.0.0.1:") {
            let port = rest.parse::<u16>()?;
            return Ok(port);
        }
    }

    Err("Server exited without outputting listening port".into())
}

/// Test harness that spawns a reovim server for integration testing
pub struct ServerTestHarness {
    process: Child,
    port: u16,
}

impl ServerTestHarness {
    /// Spawn a new server on an OS-assigned port
    ///
    /// Uses `--server --test` mode so the server exits when all clients disconnect.
    /// The server binds to port 0 and the OS assigns an available port, eliminating
    /// port contention when running tests in parallel.
    ///
    /// # Errors
    ///
    /// Returns an error if the server process fails to spawn or doesn't output
    /// its listening port within 10 seconds.
    pub async fn spawn() -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        // Inherit REOVIM_LOG from test environment for debug tracing
        let log_level = std::env::var("REOVIM_LOG").unwrap_or_else(|_| "info".to_string());

        // Use PID + counter for unique log file naming (no external dependency needed)
        let test_id = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
        let log_path = format!("/tmp/reovim-test-{}-{test_id}.log", std::process::id());

        let mut process = Command::new(binary_path())
            .args([
                "--server",
                "--test",
                "--listen-tcp",
                "0", // OS assigns port
                "--log",
                &log_path,
            ])
            .env("REOVIM_LOG", log_level)
            .env("REOVIM_TEST", "1") // Disable LSP auto-start in tests
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .kill_on_drop(true) // Automatic cleanup
            .spawn()?;

        // Read actual port from stderr with timeout
        let port = tokio::time::timeout(Duration::from_secs(10), read_port_from_stderr(&mut process))
            .await
            .map_err(|_| "Server startup timed out (10s)")?
            .map_err(|e| format!("Failed to read port from server: {e}"))?;

        Ok(Self { process, port })
    }

    /// Spawn with initial file content
    ///
    /// Uses OS-assigned port like `spawn()`.
    ///
    /// # Errors
    ///
    /// Returns an error if the server process fails to spawn or doesn't output
    /// its listening port within 10 seconds.
    pub async fn spawn_with_file(
        path: &str,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        // Inherit REOVIM_LOG from test environment for debug tracing
        let log_level = std::env::var("REOVIM_LOG").unwrap_or_else(|_| "info".to_string());

        // Use PID + counter for unique log file naming
        let test_id = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
        let log_path = format!("/tmp/reovim-test-{}-{test_id}.log", std::process::id());

        let mut process = Command::new(binary_path())
            .args([
                "--server",
                "--test",
                "--listen-tcp",
                "0", // OS assigns port
                "--log",
                &log_path,
                path,
            ])
            .env("REOVIM_LOG", log_level)
            .env("REOVIM_TEST", "1") // Disable LSP auto-start in tests
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .kill_on_drop(true) // Automatic cleanup
            .spawn()?;

        // Read actual port from stderr with timeout
        let port = tokio::time::timeout(Duration::from_secs(10), read_port_from_stderr(&mut process))
            .await
            .map_err(|_| "Server startup timed out (10s)")?
            .map_err(|e| format!("Failed to read port from server: {e}"))?;

        Ok(Self { process, port })
    }

    /// Get a connected test client
    ///
    /// # Errors
    ///
    /// Returns an error if the connection fails.
    pub async fn client(&self) -> Result<TestClient, Box<dyn std::error::Error + Send + Sync>> {
        // Retry connection with exponential backoff in case server isn't ready yet
        // This is more resilient on slower machines or under system load
        for attempt in 0..20 {
            if let Ok(client) = TestClient::connect("127.0.0.1", self.port).await {
                return Ok(client);
            }
            // Exponential backoff: 50ms, 100ms, 150ms, ..., up to 200ms
            let delay = std::cmp::min(50 + attempt * 50, 200);
            tokio::time::sleep(Duration::from_millis(delay)).await;
        }
        TestClient::connect("127.0.0.1", self.port)
            .await
            .map_err(std::convert::Into::into)
    }

    /// Get the port this server is listening on
    #[must_use]
    pub const fn port(&self) -> u16 {
        self.port
    }
}

impl Drop for ServerTestHarness {
    fn drop(&mut self) {
        // kill_on_drop(true) handles cleanup, but we still wait for the process
        // to ensure clean shutdown
        if let Ok(Some(_)) = self.process.try_wait() {
            // Process already exited
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_harness_spawn_and_connect() {
        let harness = ServerTestHarness::spawn().await.unwrap();
        assert!(harness.port() > 0);
        let _client = harness.client().await.unwrap();
        // Basic connectivity test - if we get here, connection succeeded
    }
}
