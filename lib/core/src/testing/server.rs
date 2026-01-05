//! Server test harness for integration testing
//!
//! Spawns a reovim server process and provides a client for testing.

use std::{
    path::PathBuf,
    process::{Child, Command, Stdio},
    sync::atomic::{AtomicU16, Ordering},
    time::Duration,
};

use tokio::time::sleep;

use super::client::TestClient;

/// Port range for tests: 17000-17099
static TEST_PORT: AtomicU16 = AtomicU16::new(17000);

/// Get the next available test port
fn next_test_port() -> u16 {
    let port = TEST_PORT.fetch_add(1, Ordering::SeqCst);
    // Wrap around if we exceed the range
    if port > 17099 {
        TEST_PORT.store(17000, Ordering::SeqCst);
        17000
    } else {
        port
    }
}

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

/// Test harness that spawns a reovim server for integration testing
pub struct ServerTestHarness {
    process: Child,
    port: u16,
}

impl ServerTestHarness {
    /// Spawn a new server on a unique port
    ///
    /// Uses `--server --test` mode so the server exits when all clients disconnect.
    ///
    /// # Errors
    ///
    /// Returns an error if the server process fails to spawn.
    pub async fn spawn() -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let port = next_test_port();

        // Inherit REOVIM_LOG from test environment for debug tracing
        let log_level = std::env::var("REOVIM_LOG").unwrap_or_else(|_| "info".to_string());

        let process = Command::new(binary_path())
            .args(["--server", "--test", "--listen-tcp", &port.to_string()])
            .env("REOVIM_LOG", log_level)
            .env("REOVIM_TEST", "1") // Disable LSP auto-start in tests
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()?;

        // Wait for server to be ready (longer on slower machines)
        sleep(Duration::from_millis(200)).await;

        Ok(Self { process, port })
    }

    /// Spawn with initial file content
    ///
    /// # Errors
    ///
    /// Returns an error if the server process fails to spawn.
    pub async fn spawn_with_file(
        path: &str,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let port = next_test_port();

        // Inherit REOVIM_LOG from test environment for debug tracing
        let log_level = std::env::var("REOVIM_LOG").unwrap_or_else(|_| "info".to_string());

        let process = Command::new(binary_path())
            .args([
                "--server",
                "--test",
                "--listen-tcp",
                &port.to_string(),
                path,
            ])
            .env("REOVIM_LOG", log_level)
            .env("REOVIM_TEST", "1") // Disable LSP auto-start in tests
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()?;

        // Wait for server to be ready (longer on slower machines)
        sleep(Duration::from_millis(200)).await;

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
            sleep(Duration::from_millis(delay)).await;
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
        // Kill the server process when the harness is dropped
        let _ = self.process.kill();
        let _ = self.process.wait();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_port_allocation() {
        let port1 = next_test_port();
        let port2 = next_test_port();
        assert_ne!(port1, port2);
        assert!((17000..=17099).contains(&port1));
        assert!((17000..=17099).contains(&port2));
    }
}
