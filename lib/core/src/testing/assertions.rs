//! Test builder and assertions for server-based integration tests
//!
//! Provides a fluent API for running tests against a real server.

use std::time::Duration;

use super::{client::ModeInfo, server::ServerTestHarness};

/// Builder for running server-based tests
pub struct ServerTest {
    harness: ServerTestHarness,
    initial_content: Option<String>,
    keys: Vec<String>,
}

impl ServerTest {
    /// Create a new server test
    ///
    /// Spawns a fresh server for this test.
    ///
    /// # Panics
    ///
    /// Panics if the server fails to spawn.
    pub async fn new() -> Self {
        Self {
            harness: ServerTestHarness::spawn()
                .await
                .expect("Failed to spawn server"),
            initial_content: None,
            keys: Vec::new(),
        }
    }

    /// Set initial buffer content
    #[must_use]
    pub fn with_content(mut self, content: &str) -> Self {
        self.initial_content = Some(content.to_string());
        self
    }

    /// Add key sequence to inject
    #[must_use]
    pub fn with_keys(mut self, keys: &str) -> Self {
        self.keys.push(keys.to_string());
        self
    }

    /// Run the test and return result for assertions
    ///
    /// # Panics
    ///
    /// Panics if any operation fails.
    pub async fn run(self) -> ServerTestResult {
        let mut client = self
            .harness
            .client()
            .await
            .expect("Failed to connect to server");

        // Set initial content
        if let Some(content) = &self.initial_content {
            client
                .set_buffer_content(content)
                .await
                .expect("Failed to set buffer content");
        }

        // Inject keys
        for keys in &self.keys {
            client.keys(keys).await.expect("Failed to inject keys");
        }

        // Small delay for processing
        tokio::time::sleep(Duration::from_millis(50)).await;

        // Gather results
        let mode = client.mode().await.expect("Failed to get mode");
        let cursor = client.cursor().await.ok();
        let buffer_content = client.buffer_content().await.unwrap_or_default();

        ServerTestResult {
            mode,
            cursor,
            buffer_content,
            _harness: self.harness, // Keep alive until assertions done
        }
    }
}

/// Result of a server-based test run
pub struct ServerTestResult {
    /// Final mode state
    pub mode: ModeInfo,
    /// Final cursor position (x, y)
    pub cursor: Option<(u16, u16)>,
    /// Final buffer content
    pub buffer_content: String,
    /// Keep server alive until assertions are done
    _harness: ServerTestHarness,
}

impl ServerTestResult {
    /// Assert that the buffer contains the expected content
    ///
    /// # Panics
    ///
    /// Panics if the buffer does not contain the expected string.
    pub fn assert_buffer_contains(&self, expected: &str) {
        assert!(
            self.buffer_content.contains(expected),
            "Buffer does not contain '{}'\nActual buffer content:\n{}",
            expected,
            self.buffer_content
        );
    }

    /// Assert that the buffer equals the expected content exactly
    ///
    /// # Panics
    ///
    /// Panics if the buffer content does not match exactly.
    pub fn assert_buffer_eq(&self, expected: &str) {
        assert_eq!(self.buffer_content.trim_end(), expected.trim_end(), "Buffer content mismatch");
    }

    /// Assert that the cursor is at the expected position
    ///
    /// # Panics
    ///
    /// Panics if the cursor position does not match.
    pub fn assert_cursor(&self, x: u16, y: u16) {
        assert_eq!(
            self.cursor,
            Some((x, y)),
            "Cursor position mismatch: expected ({}, {}), got {:?}",
            x,
            y,
            self.cursor
        );
    }

    /// Assert that the editor is in normal mode
    ///
    /// # Panics
    ///
    /// Panics if not in normal mode.
    pub fn assert_normal_mode(&self) {
        assert_eq!(self.mode.edit_mode, "Normal", "Expected normal mode, got {:?}", self.mode);
    }

    /// Assert that the editor is in insert mode
    ///
    /// # Panics
    ///
    /// Panics if not in insert mode.
    pub fn assert_insert_mode(&self) {
        assert!(
            self.mode.edit_mode.starts_with("Insert"),
            "Expected insert mode, got {:?}",
            self.mode
        );
    }

    /// Assert that the editor is in visual mode
    ///
    /// # Panics
    ///
    /// Panics if not in visual mode.
    pub fn assert_visual_mode(&self) {
        assert!(
            self.mode.edit_mode.starts_with("Visual"),
            "Expected visual mode, got {:?}",
            self.mode
        );
    }

    /// Assert that the editor is in command mode
    ///
    /// # Panics
    ///
    /// Panics if not in command mode.
    pub fn assert_command_mode(&self) {
        assert_eq!(self.mode.sub_mode, "Command", "Expected command mode, got {:?}", self.mode);
    }
}
