//! Multi-client test utilities for concurrent client testing.
//!
//! Tests scenarios where multiple clients connect to the same server,
//! such as collaborative editing and state synchronization.
//!
//! # Protocol
//!
//! This module uses **gRPC v2** for communication with the server.

// Test infrastructure - suppress pedantic docs requirements
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::missing_panics_doc)]

use std::{
    io::Write,
    sync::atomic::{AtomicU32, Ordering},
    time::Duration,
};

use reovim_client_cli::GrpcClient;

use super::harness::TestServerHarness;

/// Counter for unique temp file names
static MULTI_CLIENT_FILE_COUNTER: AtomicU32 = AtomicU32::new(0);

/// Client wrapper for multi-client tests.
///
/// Each method uses a persistent gRPC connection.
pub struct TestClient {
    addr: String,
    id: usize,
    client: Option<GrpcClient>,
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl TestClient {
    /// Get or create a gRPC connection.
    async fn get_client(&mut self) -> Result<&mut GrpcClient, String> {
        if self.client.is_none() {
            self.client = Some(Self::connect_with_retry(&self.addr).await?);
        }
        Ok(self.client.as_mut().unwrap())
    }

    /// Connect to server with retry logic.
    async fn connect_with_retry(addr: &str) -> Result<GrpcClient, String> {
        let mut attempts = 0;
        loop {
            match GrpcClient::connect(addr).await {
                Ok(c) => return Ok(c),
                Err(_) if attempts < 20 => {
                    attempts += 1;
                    tokio::time::sleep(Duration::from_millis(50)).await;
                }
                Err(e) => return Err(format!("Failed to connect: {e}")),
            }
        }
    }

    /// Send keys to the server.
    #[allow(clippy::significant_drop_tightening)]
    pub async fn send_keys(&mut self, keys: &str) -> Result<(), String> {
        let client = self.get_client().await?;
        client.send_keys(keys).await.map_err(|e| e.to_string())?;
        Ok(())
    }

    /// Get cursor position (line, col).
    ///
    /// (#753) `get_cursor` removed; extracted from `get_projections` "text.cursor" tag.
    #[allow(clippy::significant_drop_tightening)]
    pub async fn get_cursor(&mut self) -> Result<(u16, u16), String> {
        let client = self.get_client().await?;
        let result = client
            .get_projections(0, vec!["text.cursor".to_string()])
            .await
            .map_err(|e| e.to_string())?;
        for p in &result.projections {
            if p.tag == "text.cursor" {
                let display = p
                    .datum
                    .as_ref()
                    .and_then(|d| d.display.as_deref())
                    .unwrap_or("");
                if let Some((l, c)) = display.split_once(':') {
                    let line: u16 = l.parse().unwrap_or(0);
                    let col: u16 = c.parse().unwrap_or(0);
                    return Ok((line, col));
                }
            }
        }
        Ok((0, 0))
    }

    /// Get buffer content.
    #[allow(clippy::significant_drop_tightening)]
    #[allow(clippy::unused_async)] // async kept for API compatibility with callers that .await
    pub async fn get_buffer(&mut self) -> Result<String, String> {
        // v3: buffer content routes through projections, not GetRawContent
        Err(
            "get_buffer_content removed in proto v3; buffer content routes through projections"
                .to_string(),
        )
    }

    /// Open a new buffer with optional content.
    #[allow(clippy::significant_drop_tightening)]
    pub async fn open_buffer(&mut self, content: &str) -> Result<(), String> {
        let path = format!("/tmp/reovim-multiclient-{}-{}.txt", std::process::id(), self.id);
        let mut file = std::fs::File::create(&path).map_err(|e| e.to_string())?;
        file.write_all(content.as_bytes())
            .map_err(|e| e.to_string())?;

        let client = self.get_client().await?;
        client
            .send_keys(&format!(":e {path}<CR>"))
            .await
            .map_err(|e| e.to_string())?;

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

#[cfg_attr(coverage_nightly, coverage(off))]
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
    #[allow(clippy::significant_drop_tightening)]
    pub async fn run<F, Fut>(self, test_fn: F)
    where
        F: FnOnce(Vec<TestClient>) -> Fut,
        Fut: std::future::Future<Output = ()>,
    {
        let addr = format!("127.0.0.1:{}", self.harness.port());

        // Create test clients
        let mut clients = Vec::with_capacity(self.client_count);
        for id in 0..self.client_count {
            clients.push(TestClient {
                addr: addr.clone(),
                id,
                client: None,
            });
        }

        // Verify connectivity
        for client in &mut clients {
            assert!(
                TestClient::connect_with_retry(&client.addr).await.is_ok(),
                "Client {} failed to connect",
                client.id
            );
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
                .send_keys(&format!(":e {temp_path}<CR>"))
                .await
                .expect("Failed to open buffer file");
            tokio::time::sleep(Duration::from_millis(10)).await;
        }

        test_fn(clients).await;
        // harness dropped here, server cleaned up
    }
}
