//! Presence testing utilities for multi-client E2E tests.
//!
//! Provides `PresenceTestClient` for presence-aware integration testing,
//! wrapping the CLI's `GrpcClient` with convenient presence operations.
//!
//! # Protocol
//!
//! This module uses **gRPC v2** for communication with the server.
//!
//! # Example
//!
//! ```ignore
//! use reovim_testing::{MultiClientPresenceTest, PresenceTestClient};
//!
//! MultiClientPresenceTest::with_clients(2)
//!     .await
//!     .run(|mut clients| async move {
//!         let peers = clients[1].peers().await.unwrap();
//!         assert_eq!(peers.len(), 1);
//!         assert_eq!(peers[0].display_name, "client_0");
//!     })
//!     .await;
//! ```

// Test infrastructure - suppress pedantic docs requirements
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::missing_panics_doc)]

use std::time::Duration;

use {reovim_client_cli::GrpcClient, reovim_protocol::v2::ClientInfo};

use super::harness::TestServerHarness;

/// Presence-aware test client wrapper.
///
/// Provides convenient methods for presence operations during E2E testing.
/// Each client maintains its own gRPC connection and client ID.
pub struct PresenceTestClient {
    client: GrpcClient,
    client_id: Option<u64>,
    display_name: String,
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl PresenceTestClient {
    /// Create a new presence test client (not yet joined).
    ///
    /// Call `join()` to register with the presence service.
    pub fn new(client: GrpcClient, display_name: &str) -> Self {
        Self {
            client,
            client_id: None,
            display_name: display_name.to_string(),
        }
    }

    /// Join the presence session.
    ///
    /// Registers this client with the server and receives an assigned client ID.
    /// Returns the full join response including peer list.
    pub async fn join(&mut self) -> Result<reovim_protocol::v2::JoinResponse, String> {
        let response = self
            .client
            .presence_join("test", &self.display_name)
            .await
            .map_err(|e| format!("Join failed: {e}"))?;
        self.client_id = Some(response.client_id);
        Ok(response)
    }

    /// Leave the presence session.
    ///
    /// Unregisters this client from the server. Safe to call multiple times.
    pub async fn leave(&mut self) -> Result<(), String> {
        if self.client_id.take().is_some() {
            // Identity resolved from session token (#483)
            self.client
                .presence_leave()
                .await
                .map_err(|e| format!("Leave failed: {e}"))?;
        }
        Ok(())
    }

    /// Get client ID (panics if not joined).
    ///
    /// # Panics
    ///
    /// Panics if `join()` has not been called or if the client has left.
    #[must_use]
    #[allow(clippy::missing_const_for_fn)] // expect() is not const
    pub fn client_id(&self) -> u64 {
        self.client_id
            .expect("Client not joined - call join() first")
    }

    /// Check if this client has joined.
    #[must_use]
    pub const fn is_joined(&self) -> bool {
        self.client_id.is_some()
    }

    /// Get this client's display name.
    #[must_use]
    pub fn display_name(&self) -> &str {
        &self.display_name
    }

    /// Update cursor position.
    ///
    /// Note: This is now a no-op (Phase 14, #471).
    /// Cursor tracking moved to `CursorMoved` notifications with `client_id`.
    /// This method is preserved for test compatibility but does nothing.
    #[allow(clippy::unused_async)] // API compatibility - async signature preserved
    pub async fn update_cursor(&mut self, _line: u64, _col: u64) -> Result<(), String> {
        // Cursor is no longer tracked via presence - it's tracked via CursorMoved notifications
        Ok(())
    }

    /// Update buffer ID.
    pub async fn update_buffer(&mut self, buffer_id: u64) -> Result<(), String> {
        // Identity resolved from session token (#483)
        self.client
            .presence_update(Some(buffer_id), None)
            .await
            .map_err(|e| format!("Update buffer failed: {e}"))?;
        Ok(())
    }

    /// Update mode.
    pub async fn update_mode(&mut self, mode: &str) -> Result<(), String> {
        // Identity resolved from session token (#483)
        self.client
            .presence_update(None, Some(mode.to_string()))
            .await
            .map_err(|e| format!("Update mode failed: {e}"))?;
        Ok(())
    }

    /// Set follow mode (sync mode = 1).
    pub async fn follow(&mut self, target_id: u64) -> Result<(), String> {
        // Identity resolved from session token (#483)
        self.client
            .presence_set_sync_mode(1, Some(target_id))
            .await
            .map_err(|e| format!("Set follow mode failed: {e}"))?;
        Ok(())
    }

    /// Set present mode (sync mode = 2).
    pub async fn present(&mut self) -> Result<(), String> {
        // Identity resolved from session token (#483)
        self.client
            .presence_set_sync_mode(2, None)
            .await
            .map_err(|e| format!("Set present mode failed: {e}"))?;
        Ok(())
    }

    /// Set independent mode (sync mode = 0).
    pub async fn independent(&mut self) -> Result<(), String> {
        // Identity resolved from session token (#483)
        self.client
            .presence_set_sync_mode(0, None)
            .await
            .map_err(|e| format!("Set independent mode failed: {e}"))?;
        Ok(())
    }

    /// Get all peers (excludes self).
    pub async fn peers(&mut self) -> Result<Vec<ClientInfo>, String> {
        let response = self
            .client
            .presence_list()
            .await
            .map_err(|e| format!("List peers failed: {e}"))?;
        let my_id = self.client_id;
        Ok(response
            .clients
            .into_iter()
            .filter(|c| Some(c.id) != my_id)
            .collect())
    }

    /// Get all clients including self.
    pub async fn all_clients(&mut self) -> Result<Vec<ClientInfo>, String> {
        let response = self
            .client
            .presence_list()
            .await
            .map_err(|e| format!("List clients failed: {e}"))?;
        Ok(response.clients)
    }

    /// Access underlying gRPC client for other operations (e.g., `send_keys`).
    #[allow(clippy::missing_const_for_fn)] // mutable reference prevents const
    pub fn grpc(&mut self) -> &mut GrpcClient {
        &mut self.client
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Per-client state (#471): Per-client state isolation helpers
    // ─────────────────────────────────────────────────────────────────────────

    /// Send keys using this client's ID for per-client mode routing.
    ///
    /// # Per-client state (#471): Per-client input routing
    ///
    /// Keys are processed using this client's per-client mode stack,
    /// enabling multi-client mode isolation.
    pub async fn send_keys(&mut self, keys: &str) -> Result<(), String> {
        // Identity resolved from session token (#483)
        self.client
            .send_keys(keys)
            .await
            .map_err(|e| format!("Send keys failed: {e}"))?;
        Ok(())
    }

    /// Get the current mode display string for this client.
    ///
    /// (#753) `get_mode_for_client` removed; extracted from `get_projections` "text.mode" tag.
    pub async fn get_mode(&mut self) -> Result<String, String> {
        let result = self
            .client
            .get_projections(0, vec!["text.mode".to_string()])
            .await
            .map_err(|e| format!("Get mode failed: {e}"))?;
        let display = result
            .projections
            .iter()
            .find(|p| p.tag == "text.mode")
            .and_then(|p| p.datum.as_ref())
            .and_then(|d| d.display.clone())
            .unwrap_or_default();
        Ok(display)
    }

    /// Get cursor position for this client.
    ///
    /// (#753) `get_cursor_for_client` removed; extracted from `get_projections` "text.cursor" tag.
    pub async fn get_cursor(&mut self) -> Result<(u64, u64), String> {
        let result = self
            .client
            .get_projections(0, vec!["text.cursor".to_string()])
            .await
            .map_err(|e| format!("Get cursor failed: {e}"))?;
        for p in &result.projections {
            if p.tag == "text.cursor" {
                let display = p
                    .datum
                    .as_ref()
                    .and_then(|d| d.display.as_deref())
                    .unwrap_or("");
                if let Some((l, c)) = display.split_once(':') {
                    let line: u64 = l.parse().unwrap_or(0);
                    let col: u64 = c.parse().unwrap_or(0);
                    return Ok((line, col));
                }
            }
        }
        Ok((0, 0))
    }

    /// Get buffer content.
    ///
    /// Returns the content of the active buffer.
    pub async fn get_buffer(&mut self) -> Result<String, String> {
        Err("get_buffer_content removed in proto v3; buffer content routes through projections"
            .to_string())
    }
}

/// Multi-client test with automatic presence join/leave.
///
/// Each client automatically joins on setup with display names `client_0`,
/// `client_1`, etc. All clients are cleaned up on test completion.
///
/// # Example
///
/// ```ignore
/// MultiClientPresenceTest::with_clients(2)
///     .await
///     .run(|mut clients| async move {
///         // clients[0] and clients[1] are already joined
///         let peers = clients[1].peers().await.unwrap();
///         assert_eq!(peers.len(), 1);
///     })
///     .await;
/// ```
pub struct MultiClientPresenceTest {
    harness: TestServerHarness,
    client_count: usize,
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl MultiClientPresenceTest {
    /// Create test with N presence-aware clients.
    ///
    /// Server logs are captured to `tmp/test-logs/{test_name}_presence_{timestamp}.log`.
    ///
    /// # Panics
    ///
    /// Panics if server fails to spawn.
    pub async fn with_clients(n: usize) -> Self {
        let test_name = std::thread::current()
            .name()
            .unwrap_or("unknown_presence_test")
            .to_string();

        Self {
            harness: TestServerHarness::spawn_with_name(&format!("{test_name}_presence"))
                .await
                .expect("Failed to spawn server"),
            client_count: n,
        }
    }

    /// Get the path to the server log file for debugging.
    #[must_use]
    pub fn log_path(&self) -> Option<&std::path::Path> {
        self.harness.log_path()
    }

    /// Get the server port.
    #[must_use]
    #[allow(clippy::missing_const_for_fn)] // harness.port() is not const
    pub fn port(&self) -> u16 {
        self.harness.port()
    }

    /// Run test with presence-aware clients.
    ///
    /// Each client is auto-joined with name `client_0`, `client_1`, etc.
    /// All clients auto-leave when test completes.
    #[allow(clippy::significant_drop_tightening)]
    pub async fn run<F, Fut>(self, test_fn: F)
    where
        F: FnOnce(Vec<PresenceTestClient>) -> Fut,
        Fut: std::future::Future<Output = ()>,
    {
        let addr = format!("127.0.0.1:{}", self.harness.port());
        let mut clients = Vec::with_capacity(self.client_count);

        // Create and join all clients
        for i in 0..self.client_count {
            let grpc = Self::connect_with_retry(&addr)
                .await
                .expect("Failed to connect");
            let mut client = PresenceTestClient::new(grpc, &format!("client_{i}"));
            client.join().await.expect("Failed to join presence");
            // Small delay to ensure server processes the join
            tokio::time::sleep(Duration::from_millis(5)).await;
            clients.push(client);
        }

        // Run test
        test_fn(clients).await;

        // Cleanup is automatic when harness is dropped
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
                Err(e) => return Err(format!("Failed to connect after {attempts} attempts: {e}")),
            }
        }
    }
}
