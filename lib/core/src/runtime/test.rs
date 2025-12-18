//! Test runtime for integration testing
//!
//! Provides `TestRuntime` - a controlled environment for running
//! integration tests with mock input and captured output.

use std::time::Duration;

use tokio::time::timeout;

use crate::buffer::{Buffer, TextOps};
use crate::event::{CommandHandler, CompletionHandler, InnerEvent, InputEventBroker, TerminateHandler};
use crate::io::input::MockKeySource;
use crate::io::output::MockOutput;
use crate::modd::ModeState;
use crate::screen::Screen;

use super::Runtime;

/// Builder for configuring a test runtime.
pub struct TestRuntimeBuilder {
    width: u16,
    height: u16,
    initial_content: Option<String>,
    key_events: Vec<reovim_sys::event::KeyEvent>,
    timeout_ms: u64,
}

impl Default for TestRuntimeBuilder {
    fn default() -> Self {
        Self {
            width: 80,
            height: 24,
            initial_content: None,
            key_events: Vec::new(),
            timeout_ms: 5000,
        }
    }
}

impl TestRuntimeBuilder {
    /// Create a new test runtime builder with default settings.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the screen dimensions.
    #[must_use]
    pub const fn with_size(mut self, width: u16, height: u16) -> Self {
        self.width = width;
        self.height = height;
        self
    }

    /// Set initial buffer content.
    #[must_use]
    pub fn with_content(mut self, content: &str) -> Self {
        self.initial_content = Some(content.to_string());
        self
    }

    /// Set the key events to simulate.
    #[must_use]
    pub fn with_keys(mut self, keys: Vec<reovim_sys::event::KeyEvent>) -> Self {
        self.key_events = keys;
        self
    }

    /// Set the timeout for the test run (in milliseconds).
    #[must_use]
    pub const fn with_timeout_ms(mut self, timeout_ms: u64) -> Self {
        self.timeout_ms = timeout_ms;
        self
    }

    /// Build the test runtime.
    #[must_use]
    pub fn build(self) -> TestRuntime {
        let output = MockOutput::new();
        let screen = Screen::with_writer(output.clone(), self.width, self.height);
        let runtime = Runtime::new(screen);

        TestRuntime {
            runtime,
            output,
            key_events: self.key_events,
            initial_content: self.initial_content,
            timeout_ms: self.timeout_ms,
        }
    }
}

/// A controlled test runtime for integration testing.
pub struct TestRuntime {
    runtime: Runtime,
    output: MockOutput,
    key_events: Vec<reovim_sys::event::KeyEvent>,
    initial_content: Option<String>,
    timeout_ms: u64,
}

impl TestRuntime {
    /// Create a new test runtime builder.
    #[must_use]
    pub fn builder() -> TestRuntimeBuilder {
        TestRuntimeBuilder::new()
    }

    /// Run the test with the configured key events.
    ///
    /// Returns a `TestResult` containing the final state and output.
    #[allow(clippy::future_not_send)]
    pub async fn run(mut self) -> TestResult {
        // Set up initial buffer content
        let mut buffer = Buffer::empty(0);
        if let Some(content) = &self.initial_content {
            buffer.set_content(content);
        }
        self.runtime.buffers.insert(0, buffer);

        // Create mock key source
        let key_source = MockKeySource::new(std::mem::take(&mut self.key_events));
        let input_broker = InputEventBroker::with_key_source(key_source);

        // Set up handlers
        let mode_rx = self.runtime.subscribe_mode();
        let completion_active_rx = self.runtime.subscribe_completion_active();
        let mut command_hdr = CommandHandler::new(
            self.runtime.tx.clone(),
            mode_rx,
            completion_active_rx,
            self.runtime.command_registry.clone(),
        );
        let mut terminate_hdr = TerminateHandler::new(self.runtime.tx.clone());

        // Also set up completion handler for realistic tests
        let completion_mode_rx = self.runtime.subscribe_mode();
        let mut completion_hdr =
            CompletionHandler::with_defaults(self.runtime.tx.clone(), completion_mode_rx);

        // Enlist handlers with the key broker
        input_broker.key_broker.enlist(&mut command_hdr);
        input_broker.key_broker.enlist(&mut terminate_hdr);
        input_broker.key_broker.enlist(&mut completion_hdr);

        // Spawn handlers
        tokio::spawn(async move { command_hdr.run().await });
        tokio::spawn(async move { terminate_hdr.run().await });
        tokio::spawn(async move { completion_hdr.run().await });
        tokio::spawn(async move { input_broker.subscribe().await });

        // Initial render
        self.runtime.render();

        // Run event loop with timeout
        let timeout_duration = Duration::from_millis(self.timeout_ms);
        let result = timeout(timeout_duration, self.run_event_loop_until_idle()).await;

        let timed_out = result.is_err();

        // Extract final state
        let mode = self.runtime.mode_state.clone();
        let buffer_content = self
            .runtime
            .buffers
            .get(&self.runtime.active_buffer_id)
            .map(Buffer::content_to_string)
            .unwrap_or_default();
        let cursor_position = self
            .runtime
            .buffers
            .get(&self.runtime.active_buffer_id)
            .map(|b| (b.cur.x, b.cur.y));

        TestResult {
            output: self.output,
            mode,
            buffer_content,
            cursor_position,
            timed_out,
        }
    }

    /// Run the event loop until no more events arrive within the idle timeout.
    #[allow(clippy::future_not_send)]
    async fn run_event_loop_until_idle(&mut self) {
        let idle_timeout = Duration::from_millis(100);

        loop {
            tokio::select! {
                next = self.runtime.rx.recv() => {
                    if let Some(ev) = next {
                        if self.handle_event(ev) {
                            break; // KillSignal received
                        }
                    } else {
                        break; // Channel closed
                    }
                }
                () = tokio::time::sleep(idle_timeout) => {
                    // No events for idle_timeout - consider test done
                    break;
                }
            }
        }
    }

    /// Handle a single event.
    fn handle_event(&mut self, ev: InnerEvent) -> bool {
        self.runtime.handle_event(ev)
    }
}

/// Result of a test run.
#[derive(Debug)]
pub struct TestResult {
    /// Captured output from the screen.
    pub output: MockOutput,
    /// Final mode state.
    pub mode: ModeState,
    /// Final buffer content.
    pub buffer_content: String,
    /// Final cursor position (x, y).
    pub cursor_position: Option<(u16, u16)>,
    /// Whether the test timed out.
    pub timed_out: bool,
}

impl TestResult {
    /// Assert that the buffer contains the expected content.
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

    /// Assert that the buffer equals the expected content exactly.
    ///
    /// # Panics
    ///
    /// Panics if the buffer content does not match exactly.
    pub fn assert_buffer_eq(&self, expected: &str) {
        assert_eq!(
            self.buffer_content.trim_end(),
            expected.trim_end(),
            "Buffer content mismatch"
        );
    }

    /// Assert that the rendered output contains a string (ignoring ANSI codes).
    ///
    /// # Panics
    ///
    /// Panics if the output does not contain the expected string.
    pub fn assert_output_contains(&self, expected: &str) {
        assert!(
            self.output.contains(expected),
            "Output does not contain '{}'\nStripped output:\n{}",
            expected,
            self.output.strip_ansi()
        );
    }

    /// Assert that the editor is in the expected mode.
    ///
    /// # Panics
    ///
    /// Panics if the mode does not match.
    pub fn assert_mode(&self, expected: &ModeState) {
        assert_eq!(
            &self.mode, expected,
            "Mode mismatch: expected {:?}, got {:?}",
            expected, self.mode
        );
    }

    /// Assert that the editor is in normal mode.
    ///
    /// # Panics
    ///
    /// Panics if not in normal mode.
    pub fn assert_normal_mode(&self) {
        assert!(
            self.mode.is_normal(),
            "Expected normal mode, got {:?}",
            self.mode
        );
    }

    /// Assert that the editor is in insert mode.
    ///
    /// # Panics
    ///
    /// Panics if not in insert mode.
    pub fn assert_insert_mode(&self) {
        assert!(
            self.mode.is_insert(),
            "Expected insert mode, got {:?}",
            self.mode
        );
    }

    /// Assert that the cursor is at the expected position.
    ///
    /// # Panics
    ///
    /// Panics if the cursor position does not match.
    pub fn assert_cursor(&self, x: u16, y: u16) {
        assert_eq!(
            self.cursor_position,
            Some((x, y)),
            "Cursor position mismatch: expected ({}, {}), got {:?}",
            x,
            y,
            self.cursor_position
        );
    }

    /// Assert that the test did not time out.
    ///
    /// # Panics
    ///
    /// Panics if the test timed out.
    pub fn assert_no_timeout(&self) {
        assert!(!self.timed_out, "Test timed out");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_runtime_builder_defaults() {
        let runtime = TestRuntime::builder().build();
        assert!(runtime.key_events.is_empty());
        assert!(runtime.initial_content.is_none());
    }

    #[tokio::test]
    async fn test_runtime_builder_with_content() {
        let runtime = TestRuntime::builder()
            .with_content("hello world")
            .build();
        assert_eq!(runtime.initial_content, Some("hello world".to_string()));
    }
}
