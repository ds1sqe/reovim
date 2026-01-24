//! Multi-client test utilities for concurrent client testing.
//!
//! Tests scenarios where multiple clients connect to the same server,
//! such as collaborative editing and state synchronization.

// Test infrastructure - suppress pedantic docs requirements
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::missing_panics_doc)]

use std::{
    io::Write,
    sync::atomic::{AtomicU32, Ordering},
    time::Duration,
};

use serde_json::{Value, json};

use crate::client::common::{ConnectionConfig, RpcClient};

use super::harness::TestServerHarness;

/// Counter for unique temp file names
static MULTI_CLIENT_FILE_COUNTER: AtomicU32 = AtomicU32::new(0);

/// Client wrapper for multi-client tests.
///
/// Each method creates a fresh connection to avoid stale notification issues.
pub struct TestClient {
    config: ConnectionConfig,
    id: usize,
}

impl TestClient {
    /// Make an RPC call with a fresh connection.
    async fn call(&self, method: &str, params: Value) -> Result<Value, String> {
        let mut client = Self::connect_with_retry(&self.config).await?;
        client.call(method, params).await.map_err(|e| e.to_string())
    }

    /// Connect to server with retry logic.
    async fn connect_with_retry(config: &ConnectionConfig) -> Result<RpcClient, String> {
        let mut attempts = 0;
        loop {
            match RpcClient::connect(config).await {
                Ok(c) => return Ok(c),
                Err(e) if attempts < 20 => {
                    attempts += 1;
                    tokio::time::sleep(Duration::from_millis(50)).await;
                }
                Err(e) => return Err(format!("Failed to connect: {e}")),
            }
        }
    }

    /// Send keys to the server.
    pub async fn send_keys(&self, keys: &str) -> Result<Value, String> {
        self.call("input/keys", json!({ "keys": keys })).await
    }

    /// Get cursor position (line, col).
    #[allow(clippy::cast_possible_truncation)]
    pub async fn get_cursor(&self) -> Result<(u16, u16), String> {
        let result = self.call("state/cursor", json!({})).await?;
        Ok((
            result["line"].as_u64().unwrap_or(0) as u16,
            result["column"].as_u64().unwrap_or(0) as u16,
        ))
    }

    /// Get buffer content.
    pub async fn get_buffer(&self) -> Result<String, String> {
        let result = self.call("buffer/get_content", json!({})).await?;
        Ok(result["content"].as_str().unwrap_or("").to_string())
    }

    /// Open a new buffer with optional content.
    pub async fn open_buffer(&self, content: &str) -> Result<(), String> {
        let path = format!("/tmp/reovim-multiclient-{}-{}.txt", std::process::id(), self.id);
        let mut file = std::fs::File::create(&path).map_err(|e| e.to_string())?;
        file.write_all(content.as_bytes())
            .map_err(|e| e.to_string())?;

        self.call("buffer/open_file", json!({ "path": &path }))
            .await?;

        Ok(())
    }

    /// Get client ID.
    #[must_use]
    pub const fn id(&self) -> usize {
        self.id
    }
}

/// Multi-client test builder.
///
/// # Example
///
/// ```ignore
/// MultiClientTest::with_clients(2)
///     .await
///     .with_buffer("initial")
///     .run(|mut clients| async move {
///         clients[0].send_keys("ihello<Esc>").await.unwrap();
///         let content = clients[1].get_buffer().await.unwrap();
///         assert!(content.contains("hello"));
///     })
///     .await;
/// ```
pub struct MultiClientTest {
    harness: TestServerHarness,
    client_count: usize,
    initial_content: Option<String>,
}

impl MultiClientTest {
    /// Create test with N clients.
    ///
    /// Server logs are captured to `tmp/test-logs/{test_name}_server_{timestamp}.log`.
    ///
    /// # Panics
    ///
    /// Panics if server fails to spawn.
    pub async fn with_clients(n: usize) -> Self {
        // Extract test name with suffix for unique log files
        let test_name = std::thread::current()
            .name()
            .unwrap_or("unknown_multi_test")
            .to_string();

        Self {
            harness: TestServerHarness::spawn_with_name(&format!("{test_name}_server"))
                .await
                .expect("Failed to spawn server"),
            client_count: n,
            initial_content: None,
        }
    }

    /// Get the path to the server log file for debugging.
    #[must_use]
    pub fn log_path(&self) -> Option<&std::path::Path> {
        self.harness.log_path()
    }

    /// Set initial buffer content (shared by all clients).
    #[must_use]
    pub fn with_buffer(mut self, content: &str) -> Self {
        self.initial_content = Some(content.to_string());
        self
    }

    /// Connect all clients and run test.
    pub async fn run<F, Fut>(self, test_fn: F)
    where
        F: FnOnce(Vec<TestClient>) -> Fut,
        Fut: std::future::Future<Output = ()>,
    {
        let config = ConnectionConfig::tcp("127.0.0.1", self.harness.port());

        // Create test clients
        let mut clients = Vec::with_capacity(self.client_count);
        for id in 0..self.client_count {
            clients.push(TestClient {
                config: config.clone(),
                id,
            });
        }

        // Verify connectivity
        for client in &clients {
            if TestClient::connect_with_retry(&client.config)
                .await
                .is_err()
            {
                panic!("Client {} failed to connect", client.id);
            }
        }

        // Set up initial buffer via first client
        if self.initial_content.is_some() || !clients.is_empty() {
            let temp_path = {
                let id = MULTI_CLIENT_FILE_COUNTER.fetch_add(1, Ordering::SeqCst);
                let path = format!("/tmp/reovim-multi-test-{}-{id}.txt", std::process::id());
                let content = self.initial_content.as_deref().unwrap_or("");
                let mut file = std::fs::File::create(&path).expect("Failed to create temp file");
                file.write_all(content.as_bytes())
                    .expect("Failed to write temp file");
                path
            };
            clients[0]
                .call("buffer/open_file", json!({ "path": &temp_path }))
                .await
                .expect("Failed to open buffer file");
            tokio::time::sleep(Duration::from_millis(10)).await;
        }

        test_fn(clients).await;
        // harness dropped here, server cleaned up
    }
}
