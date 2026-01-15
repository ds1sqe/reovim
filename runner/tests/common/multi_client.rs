//! Multi-client test utilities for concurrent client testing.

use std::time::Duration;

use {
    runner::client::common::{ConnectionConfig, RpcClient},
    serde_json::{Value, json},
};

use super::harness::TestServerHarness;

/// Client wrapper for multi-client tests
pub struct TestClient {
    client: RpcClient,
    id: usize,
}

impl TestClient {
    /// Send keys
    pub async fn send_keys(&mut self, keys: &str) -> Result<Value, String> {
        self.client
            .call("input/keys", json!({ "keys": keys }))
            .await
            .map_err(|e| e.to_string())
    }

    /// Get cursor position (line, col)
    #[allow(clippy::cast_possible_truncation)] // Cursor coords fit in u16
    pub async fn get_cursor(&mut self) -> Result<(u16, u16), String> {
        let result = self
            .client
            .call("state/cursor", json!({}))
            .await
            .map_err(|e| e.to_string())?;
        Ok((
            result["line"].as_u64().unwrap_or(0) as u16,
            result["column"].as_u64().unwrap_or(0) as u16,
        ))
    }

    /// Get buffer content
    pub async fn get_buffer(&mut self) -> Result<String, String> {
        let result = self
            .client
            .call("buffer/get_content", json!({}))
            .await
            .map_err(|e| e.to_string())?;
        Ok(result["content"].as_str().unwrap_or("").to_string())
    }

    /// Client ID
    #[must_use]
    pub const fn id(&self) -> usize {
        self.id
    }
}

/// Multi-client test builder
pub struct MultiClientTest {
    harness: TestServerHarness,
    client_count: usize,
}

impl MultiClientTest {
    /// Create test with N clients
    pub async fn with_clients(n: usize) -> Self {
        Self {
            harness: TestServerHarness::spawn()
                .await
                .expect("Failed to spawn server"),
            client_count: n,
        }
    }

    /// Connect all clients and run test
    pub async fn run<F, Fut>(self, test_fn: F)
    where
        F: FnOnce(Vec<TestClient>) -> Fut,
        Fut: std::future::Future<Output = ()>,
    {
        let config = ConnectionConfig::tcp("127.0.0.1", self.harness.port());

        let mut clients = Vec::with_capacity(self.client_count);
        for id in 0..self.client_count {
            let mut attempts = 0;
            let client = loop {
                match RpcClient::connect(&config).await {
                    Ok(c) => break c,
                    Err(e) if attempts < 20 => {
                        attempts += 1;
                        tokio::time::sleep(Duration::from_millis(50)).await;
                    }
                    Err(e) => panic!("Client {id} failed to connect: {e}"),
                }
            };
            clients.push(TestClient { client, id });
        }

        test_fn(clients).await;
        // harness dropped here, server cleaned up
    }
}
