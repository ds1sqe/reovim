//! Test builder and assertions for server-based integration tests
//!
//! Provides a fluent API for running tests against a real server.

use std::time::Duration;

use {
    super::{
        client::{MicroscopeInfo, ModeInfo, ScreenInfo, TelescopeInfo, TestClient, WindowInfo},
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
        let telescope = client.telescope().await.ok();
        let microscope = client.microscope().await.ok();
        let screen_content = client.screen_content_raw().await.unwrap_or_default();
        let screen = client.screen().await.ok();

        ServerTestResult {
            mode,
            cursor,
            buffer_content,
            telescope,
            microscope,
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
    /// Telescope state
    pub telescope: Option<TelescopeInfo>,
    /// Microscope state
    pub microscope: Option<MicroscopeInfo>,
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

    // === Microscope Assertions ===

    /// Assert that microscope is active/visible
    ///
    /// # Panics
    ///
    /// Panics if microscope is not active.
    pub fn assert_microscope_active(&self) {
        let ms = self
            .microscope
            .as_ref()
            .expect("Microscope state not available");
        assert!(ms.active, "Expected microscope to be active");
    }

    /// Assert that microscope is inactive/hidden
    ///
    /// # Panics
    ///
    /// Panics if microscope is active.
    pub fn assert_microscope_inactive(&self) {
        let ms = self
            .microscope
            .as_ref()
            .expect("Microscope state not available");
        assert!(!ms.active, "Expected microscope to be inactive");
    }

    /// Assert that microscope has the specified picker
    ///
    /// # Panics
    ///
    /// Panics if the picker name doesn't match.
    pub fn assert_microscope_picker(&self, expected: &str) {
        let ms = self
            .microscope
            .as_ref()
            .expect("Microscope state not available");
        assert_eq!(ms.picker_name, expected, "Microscope picker mismatch");
    }

    /// Assert that microscope has items
    ///
    /// # Panics
    ///
    /// Panics if microscope has no items.
    pub fn assert_microscope_has_items(&self) {
        let ms = self
            .microscope
            .as_ref()
            .expect("Microscope state not available");
        assert!(ms.item_count > 0, "Expected microscope to have items, but got 0");
    }

    /// Assert that microscope query matches
    ///
    /// # Panics
    ///
    /// Panics if the query doesn't match.
    pub fn assert_microscope_query(&self, expected: &str) {
        let ms = self
            .microscope
            .as_ref()
            .expect("Microscope state not available");
        assert_eq!(ms.query, expected, "Microscope query mismatch");
    }

    /// Assert that microscope is in insert mode
    ///
    /// # Panics
    ///
    /// Panics if not in insert mode.
    pub fn assert_microscope_insert_mode(&self) {
        let ms = self
            .microscope
            .as_ref()
            .expect("Microscope state not available");
        assert_eq!(
            ms.prompt_mode, "Insert",
            "Expected microscope insert mode, got {}",
            ms.prompt_mode
        );
    }

    /// Assert that microscope is in normal mode
    ///
    /// # Panics
    ///
    /// Panics if not in normal mode.
    pub fn assert_microscope_normal_mode(&self) {
        let ms = self
            .microscope
            .as_ref()
            .expect("Microscope state not available");
        assert_eq!(
            ms.prompt_mode, "Normal",
            "Expected microscope normal mode, got {}",
            ms.prompt_mode
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

    // === Window methods ===

    /// Get all windows state
    ///
    /// Returns a list of all windows with their scroll positions, cursors, and active state.
    ///
    /// # Panics
    ///
    /// Panics if the request fails.
    pub async fn windows(&mut self) -> Vec<WindowInfo> {
        self.client.windows().await.expect("Failed to get windows")
    }

    /// Assert that a window has the expected scroll position (`buffer_anchor_y`)
    ///
    /// # Arguments
    ///
    /// * `windows` - The list of windows from `windows()` call
    /// * `window_index` - Index of the window in the list
    /// * `expected_y` - Expected vertical scroll position (buffer line at top of viewport)
    ///
    /// # Panics
    ///
    /// Panics if the window doesn't exist or scroll position doesn't match.
    pub fn assert_window_scroll(windows: &[WindowInfo], window_index: usize, expected_y: u16) {
        let window = windows
            .get(window_index)
            .unwrap_or_else(|| panic!("Window index {window_index} does not exist"));
        assert_eq!(
            window.buffer_anchor_y, expected_y,
            "Window {} scroll mismatch: expected y={}, got y={}",
            window_index, expected_y, window.buffer_anchor_y
        );
    }

    /// Find a window by its active state
    ///
    /// # Returns
    ///
    /// The first window with `is_active == true`, or None if no active window.
    #[must_use]
    pub fn find_active_window(windows: &[WindowInfo]) -> Option<&WindowInfo> {
        windows.iter().find(|w| w.is_active)
    }

    /// Find a window by its ID
    ///
    /// # Returns
    ///
    /// The window with the given ID, or None if not found.
    #[must_use]
    pub fn find_window_by_id(windows: &[WindowInfo], id: usize) -> Option<&WindowInfo> {
        windows.iter().find(|w| w.id == id)
    }
}
