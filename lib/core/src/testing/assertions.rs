//! Test builder and assertions for server-based integration tests
//!
//! Provides a fluent API for running tests against a real server.

use std::time::Duration;

use {
    super::{
        client::{ModeInfo, ScreenInfo, TelescopeInfo, TestClient, WhichKeyInfo},
        server::ServerTestHarness,
    },
    crate::visual::{LayerInfo, VisualSnapshot},
};

/// A key sequence with optional delay after it
struct KeySequence {
    keys: String,
    delay_ms: u64,
}

/// Builder for running server-based tests
pub struct ServerTest {
    harness: ServerTestHarness,
    initial_content: Option<String>,
    key_sequences: Vec<KeySequence>,
    /// Default delay between key sequences (ms)
    default_delay: u64,
    /// Explicit screen size (width, height)
    screen_size: Option<(u16, u16)>,
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
            key_sequences: Vec::new(),
            default_delay: 50,
            screen_size: None,
        }
    }

    /// Set explicit screen size (width, height)
    ///
    /// This resizes the editor to the specified dimensions before running the test.
    #[must_use]
    pub const fn with_size(mut self, width: u16, height: u16) -> Self {
        self.screen_size = Some((width, height));
        self
    }

    /// Set initial buffer content
    #[must_use]
    pub fn with_content(mut self, content: &str) -> Self {
        self.initial_content = Some(content.to_string());
        self
    }

    /// Add key sequence to inject with default delay (50ms)
    #[must_use]
    pub fn with_keys(mut self, keys: &str) -> Self {
        self.key_sequences.push(KeySequence {
            keys: keys.to_string(),
            delay_ms: self.default_delay,
        });
        self
    }

    /// Add delay after the last key sequence
    ///
    /// This modifies the delay of the most recently added key sequence.
    /// Useful for which-key timeout testing (popup appears after 500ms).
    ///
    /// # Panics
    ///
    /// Panics if called before any `with_keys()`.
    #[must_use]
    pub fn with_delay(mut self, ms: u64) -> Self {
        if let Some(last) = self.key_sequences.last_mut() {
            last.delay_ms = ms;
        }
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

        // Set screen size if specified
        if let Some((width, height)) = self.screen_size {
            client
                .resize(width, height)
                .await
                .expect("Failed to resize screen");
        }

        // Set initial content
        if let Some(content) = &self.initial_content {
            client
                .set_buffer_content(content)
                .await
                .expect("Failed to set buffer content");
        }

        // Inject keys with delays between sequences
        for seq in &self.key_sequences {
            client.keys(&seq.keys).await.expect("Failed to inject keys");
            tokio::time::sleep(Duration::from_millis(seq.delay_ms)).await;
        }

        // Gather results
        let mode = client.mode().await.expect("Failed to get mode");
        let cursor = client.cursor().await.ok();
        let buffer_content = client.buffer_content().await.unwrap_or_default();
        let whichkey = client.whichkey().await.ok();
        let telescope = client.telescope().await.ok();
        let screen_content = client.screen_content_raw().await.unwrap_or_default();
        let screen = client.screen().await.ok();

        ServerTestResult {
            mode,
            cursor,
            buffer_content,
            whichkey,
            telescope,
            screen_content,
            screen,
            client,                 // Keep client for visual methods
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
    /// Which-key panel state
    pub whichkey: Option<WhichKeyInfo>,
    /// Telescope state
    pub telescope: Option<TelescopeInfo>,
    /// Raw screen content with ANSI escape codes
    pub screen_content: String,
    /// Screen state (dimensions and active buffer)
    pub screen: Option<ScreenInfo>,
    /// Client for visual methods (async)
    client: TestClient,
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

    /// Assert that the which-key panel is visible
    ///
    /// # Panics
    ///
    /// Panics if the which-key panel is not visible.
    pub fn assert_whichkey_visible(&self) {
        let wk = self
            .whichkey
            .as_ref()
            .expect("Which-key state not available");
        assert!(wk.visible, "Expected which-key panel to be visible");
    }

    /// Assert that the which-key panel is hidden
    ///
    /// # Panics
    ///
    /// Panics if the which-key panel is visible.
    pub fn assert_whichkey_hidden(&self) {
        let wk = self
            .whichkey
            .as_ref()
            .expect("Which-key state not available");
        assert!(!wk.visible, "Expected which-key panel to be hidden");
    }

    /// Assert that the which-key panel has a specific prefix
    ///
    /// # Panics
    ///
    /// Panics if the prefix doesn't match.
    pub fn assert_whichkey_prefix(&self, expected: &str) {
        let wk = self
            .whichkey
            .as_ref()
            .expect("Which-key state not available");
        assert_eq!(wk.prefix, expected, "Which-key prefix mismatch");
    }

    /// Assert that the which-key panel has a binding with the given key
    ///
    /// # Panics
    ///
    /// Panics if no binding with the key exists.
    pub fn assert_whichkey_has_binding(&self, key: &str) {
        let wk = self
            .whichkey
            .as_ref()
            .expect("Which-key state not available");
        assert!(
            wk.bindings.iter().any(|b| b.key == key),
            "Which-key panel does not have binding for key '{}'. Available: {:?}",
            key,
            wk.bindings.iter().map(|b| &b.key).collect::<Vec<_>>()
        );
    }

    /// Assert that telescope is active/visible
    ///
    /// # Panics
    ///
    /// Panics if telescope is not active.
    pub fn assert_telescope_active(&self) {
        let ts = self
            .telescope
            .as_ref()
            .expect("Telescope state not available");
        assert!(ts.active, "Expected telescope to be active");
    }

    /// Assert that telescope is inactive/hidden
    ///
    /// # Panics
    ///
    /// Panics if telescope is active.
    pub fn assert_telescope_inactive(&self) {
        let ts = self
            .telescope
            .as_ref()
            .expect("Telescope state not available");
        assert!(!ts.active, "Expected telescope to be inactive");
    }

    /// Assert that telescope has the specified picker
    ///
    /// # Panics
    ///
    /// Panics if the picker name doesn't match.
    pub fn assert_telescope_picker(&self, expected: &str) {
        let ts = self
            .telescope
            .as_ref()
            .expect("Telescope state not available");
        assert_eq!(ts.picker_name, expected, "Telescope picker mismatch");
    }

    /// Assert that telescope has items
    ///
    /// # Panics
    ///
    /// Panics if telescope has no items.
    pub fn assert_telescope_has_items(&self) {
        let ts = self
            .telescope
            .as_ref()
            .expect("Telescope state not available");
        assert!(ts.item_count > 0, "Expected telescope to have items, but got 0");
    }

    /// Assert that telescope query matches
    ///
    /// # Panics
    ///
    /// Panics if the query doesn't match.
    pub fn assert_telescope_query(&self, expected: &str) {
        let ts = self
            .telescope
            .as_ref()
            .expect("Telescope state not available");
        assert_eq!(ts.query, expected, "Telescope query mismatch");
    }

    /// Assert that cursor visibility is properly managed during rendering
    ///
    /// Verifies that the terminal output contains Hide cursor escape code
    /// before the Show cursor escape code, ensuring cursor is hidden during
    /// screen updates to prevent blinking.
    ///
    /// # Panics
    ///
    /// Panics if Hide/Show escape codes are missing or in wrong order.
    pub fn assert_cursor_visibility_managed(&self) {
        const HIDE_CURSOR: &str = "\x1b[?25l";
        const SHOW_CURSOR: &str = "\x1b[?25h";

        let hide_pos = self.screen_content.find(HIDE_CURSOR);
        let show_pos = self.screen_content.rfind(SHOW_CURSOR);

        assert!(hide_pos.is_some(), "Hide cursor escape code not found in screen output");
        assert!(show_pos.is_some(), "Show cursor escape code not found in screen output");
        assert!(
            hide_pos.unwrap() < show_pos.unwrap(),
            "Hide cursor should appear before Show cursor in output"
        );
    }

    /// Assert that the active buffer ID matches
    ///
    /// # Panics
    ///
    /// Panics if the active buffer ID doesn't match.
    pub fn assert_active_buffer_id(&self, expected: usize) {
        let screen = self.screen.as_ref().expect("Screen state not available");
        assert_eq!(
            screen.active_buffer_id, expected,
            "Active buffer ID mismatch: expected {}, got {}",
            expected, screen.active_buffer_id
        );
    }

    /// Get the active buffer ID
    ///
    /// # Panics
    ///
    /// Panics if screen state is not available.
    #[must_use]
    #[allow(clippy::missing_const_for_fn)] // expect is not const
    pub fn active_buffer_id(&self) -> usize {
        self.screen
            .as_ref()
            .expect("Screen state not available")
            .active_buffer_id
    }

    /// Assert that the window count matches
    ///
    /// # Panics
    ///
    /// Panics if the window count doesn't match.
    pub fn assert_window_count(&self, expected: usize) {
        let screen = self.screen.as_ref().expect("Screen state not available");
        assert_eq!(
            screen.window_count, expected,
            "Window count mismatch: expected {}, got {}",
            expected, screen.window_count
        );
    }

    /// Get the current window count
    ///
    /// # Panics
    ///
    /// Panics if screen state is not available.
    #[must_use]
    #[allow(clippy::missing_const_for_fn)]
    pub fn window_count(&self) -> usize {
        self.screen
            .as_ref()
            .expect("Screen state not available")
            .window_count
    }

    // === Visual methods ===

    /// Get visual snapshot for testing
    ///
    /// Returns a structured snapshot with cell grid, cursor, and layer info.
    ///
    /// # Panics
    ///
    /// Panics if the request fails.
    pub async fn visual_snapshot(&mut self) -> VisualSnapshot {
        self.client
            .visual_snapshot()
            .await
            .expect("Failed to get visual snapshot")
    }

    /// Get ASCII art representation of the screen
    ///
    /// # Arguments
    ///
    /// * `annotated` - If true, includes borders and row/column numbers
    ///
    /// # Panics
    ///
    /// Panics if the request fails.
    pub async fn ascii_art(&mut self, annotated: bool) -> String {
        self.client
            .ascii_art(annotated)
            .await
            .expect("Failed to get ASCII art")
    }

    /// Get layer information
    ///
    /// Returns information about all layers including their z-order and bounds.
    ///
    /// # Panics
    ///
    /// Panics if the request fails.
    pub async fn layer_info(&mut self) -> Vec<LayerInfo> {
        self.client
            .layer_info()
            .await
            .expect("Failed to get layer info")
    }
}
