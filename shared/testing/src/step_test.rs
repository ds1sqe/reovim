//! Step-by-step integration test builder with per-key state tracking.
//!
//! Provides a fluent API for testing key-by-key behavior, capturing state
//! (cursor, mode, registers) after each key and allowing assertions at each step.
//!
//! # Protocol
//!
//! This module uses **gRPC v2** for communication with the server.
//!
//! # Example
//!
//! ```ignore
//! let trace = StepTest::new()
//!     .await
//!     .with_buffer("hello world")
//!     .step("d")
//!         .expect_mode_contains("delete")
//!     .step("w")
//!         .expect_buffer("world")
//!         .expect_cursor(0, 0)
//!         .expect_register("\"", "hello ", "characterwise")
//!     .run()
//!     .await;
//! trace.print_trace();
//! ```

// Test infrastructure - suppress pedantic docs requirements
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::missing_panics_doc)]

use std::{
    collections::HashMap,
    io::Write,
    sync::atomic::{AtomicU32, Ordering},
    time::Duration,
};

use reovim_client_cli::GrpcClient;

use super::{harness::TestServerHarness, integration::RegisterInfo};

/// Counter for unique temp file names
static STEP_TEMP_FILE_COUNTER: AtomicU32 = AtomicU32::new(0);

/// Connection retry settings
const MAX_CONNECT_ATTEMPTS: u32 = 20;
const RETRY_BASE_MS: u64 = 50;
const RETRY_MAX_MS: u64 = 200;

/// Default delay after each key (allows event processing)
const DEFAULT_STEP_DELAY_MS: u64 = 30;

/// State snapshot captured after a key.
#[derive(Debug, Clone)]
pub struct StateSnapshot {
    /// Key that was pressed to reach this state.
    pub key: String,
    /// Buffer content after this key.
    pub buffer: String,
    /// Cursor line (0-indexed).
    pub cursor_line: u16,
    /// Cursor column (0-indexed).
    pub cursor_column: u16,
    /// Mode display name (e.g., "NORMAL", "DELETE").
    pub mode_display: String,
    /// Edit mode string (e.g., "vim:normal", "vim:delete").
    pub edit_mode: String,
    /// Register contents.
    pub registers: HashMap<String, RegisterInfo>,
}

impl StateSnapshot {
    /// Format as compact single line: "key -> mode (line:col)"
    #[must_use]
    pub fn compact(&self) -> String {
        format!(
            "{} -> {} ({}:{})",
            self.key, self.mode_display, self.cursor_line, self.cursor_column
        )
    }

    /// Format with buffer preview.
    #[must_use]
    pub fn verbose(&self) -> String {
        let buf_preview = if self.buffer.len() > 40 {
            format!("{}...", &self.buffer[..40])
        } else {
            self.buffer.clone()
        };
        format!(
            "{} -> {} ({}:{}) buf={:?}",
            self.key, self.mode_display, self.cursor_line, self.cursor_column, buf_preview
        )
    }
}

/// Per-step expectation to be verified after the key.
#[derive(Debug, Clone)]
enum StepExpectation {
    /// Expect cursor at (line, col).
    Cursor(u16, u16),
    /// Expect buffer equals.
    Buffer(String),
    /// Expect buffer contains.
    BufferContains(String),
    /// Expect mode display contains.
    ModeContains(String),
    /// Expect edit mode equals.
    EditMode(String),
    /// Expect register content and type.
    Register(String, String, String),
}

/// A step in the test sequence.
struct TestStep {
    /// Key or key sequence to send.
    keys: String,
    /// Delay after this step.
    delay_ms: u64,
    /// Expectations to verify after this step.
    expectations: Vec<StepExpectation>,
}

/// Step-by-step test builder.
///
/// # Example
///
/// ```ignore
/// let trace = StepTest::new()
///     .await
///     .with_buffer("line 1\nline 2\nline 3")
///     .step("d")
///         .expect_mode_contains("DELETE")
///     .step("j")
///         .expect_buffer("line 3")
///         .expect_cursor(0, 0)
///     .run()
///     .await;
/// trace.print_trace();
/// ```
pub struct StepTest {
    harness: TestServerHarness,
    addr: String,
    initial_content: Option<String>,
    initial_cursor: Option<(u16, u16)>,
    steps: Vec<TestStep>,
    default_delay: u64,
}

impl StepTest {
    /// Create new step test with automatic log capture (spawns server).
    ///
    /// Server logs are captured to `tmp/test-logs/{test_name}_{timestamp}.log`.
    /// This is invaluable for debugging step-by-step test failures.
    ///
    /// # Panics
    ///
    /// Panics if server fails to spawn.
    pub async fn new() -> Self {
        let harness = TestServerHarness::spawn()
            .await
            .expect("Failed to spawn server");
        let addr = format!("127.0.0.1:{}", harness.port());
        Self {
            harness,
            addr,
            initial_content: None,
            initial_cursor: None,
            steps: Vec::new(),
            default_delay: DEFAULT_STEP_DELAY_MS,
        }
    }

    /// Get the path to the server log file for debugging.
    ///
    /// Returns `None` if log capture is not enabled.
    #[must_use]
    pub fn log_path(&self) -> Option<&std::path::Path> {
        self.harness.log_path()
    }

    /// Connect to server with retry logic using gRPC.
    async fn connect_with_retry(&self) -> Result<GrpcClient, String> {
        let mut attempts = 0;
        loop {
            match GrpcClient::connect(&self.addr).await {
                Ok(c) => return Ok(c),
                Err(_) if attempts < MAX_CONNECT_ATTEMPTS => {
                    attempts += 1;
                    let delay = std::cmp::min(
                        RETRY_BASE_MS + u64::from(attempts) * RETRY_BASE_MS,
                        RETRY_MAX_MS,
                    );
                    tokio::time::sleep(Duration::from_millis(delay)).await;
                }
                Err(e) => {
                    return Err(format!(
                        "Failed to connect after {MAX_CONNECT_ATTEMPTS} attempts: {e}"
                    ));
                }
            }
        }
    }

    /// Capture current state from server via gRPC.
    async fn capture_state(&self, client: &mut GrpcClient, key: &str) -> StateSnapshot {
        let buffer_response = client
            .get_buffer_content(None)
            .await
            .expect("Failed to get buffer content");
        let cursor_response = client.get_cursor().await.expect("Failed to get cursor");
        let mode_response = client.get_mode().await.expect("Failed to get mode");

        // Extract cursor position from nested Position message
        let (cursor_line, cursor_column) = cursor_response
            .position
            .map_or((0, 0), |pos| (pos.line, pos.column));

        // TODO: Implement register query via gRPC (Phase 9+)
        let registers = HashMap::new();

        #[allow(clippy::cast_possible_truncation)]
        StateSnapshot {
            key: key.to_string(),
            buffer: buffer_response.lines.join("\n"),
            cursor_line: cursor_line as u16,
            cursor_column: cursor_column as u16,
            mode_display: mode_response.display,
            edit_mode: mode_response.name,
            registers,
        }
    }

    /// Set initial buffer content.
    #[must_use]
    pub fn with_buffer(mut self, content: &str) -> Self {
        self.initial_content = Some(content.to_string());
        self
    }

    /// Set initial cursor position (line, col) - 0-indexed.
    #[must_use]
    #[allow(clippy::missing_const_for_fn)]
    pub fn with_cursor_at(mut self, line: u16, col: u16) -> Self {
        self.initial_cursor = Some((line, col));
        self
    }

    /// Set default delay between steps (ms).
    #[must_use]
    #[allow(clippy::missing_const_for_fn)]
    pub fn with_delay(mut self, ms: u64) -> Self {
        self.default_delay = ms;
        self
    }

    /// Add a step (key to send).
    #[must_use]
    pub fn step(mut self, keys: &str) -> Self {
        self.steps.push(TestStep {
            keys: keys.to_string(),
            delay_ms: self.default_delay,
            expectations: Vec::new(),
        });
        self
    }

    /// Add cursor expectation to last step.
    #[must_use]
    pub fn expect_cursor(mut self, line: u16, col: u16) -> Self {
        if let Some(step) = self.steps.last_mut() {
            step.expectations.push(StepExpectation::Cursor(line, col));
        }
        self
    }

    /// Add buffer content expectation to last step.
    #[must_use]
    pub fn expect_buffer(mut self, expected: &str) -> Self {
        if let Some(step) = self.steps.last_mut() {
            step.expectations
                .push(StepExpectation::Buffer(expected.to_string()));
        }
        self
    }

    /// Add buffer contains expectation to last step.
    #[must_use]
    pub fn expect_buffer_contains(mut self, substring: &str) -> Self {
        if let Some(step) = self.steps.last_mut() {
            step.expectations
                .push(StepExpectation::BufferContains(substring.to_string()));
        }
        self
    }

    /// Add mode contains expectation to last step.
    #[must_use]
    pub fn expect_mode_contains(mut self, substring: &str) -> Self {
        if let Some(step) = self.steps.last_mut() {
            step.expectations
                .push(StepExpectation::ModeContains(substring.to_string()));
        }
        self
    }

    /// Add exact edit mode expectation to last step.
    #[must_use]
    pub fn expect_edit_mode(mut self, mode: &str) -> Self {
        if let Some(step) = self.steps.last_mut() {
            step.expectations
                .push(StepExpectation::EditMode(mode.to_string()));
        }
        self
    }

    /// Add register expectation to last step.
    #[must_use]
    pub fn expect_register(mut self, reg: &str, content: &str, yank_type: &str) -> Self {
        if let Some(step) = self.steps.last_mut() {
            step.expectations.push(StepExpectation::Register(
                reg.to_string(),
                content.to_string(),
                yank_type.to_string(),
            ));
        }
        self
    }

    /// Run the test and return trace.
    ///
    /// # Panics
    ///
    /// Panics if any step expectation fails.
    #[allow(clippy::too_many_lines)]
    pub async fn run(self) -> StepTrace {
        // Create buffer via temp file
        let temp_path = {
            let id = STEP_TEMP_FILE_COUNTER.fetch_add(1, Ordering::SeqCst);
            let path = format!("/tmp/reovim-step-test-{}-{id}.txt", std::process::id());
            let content = self.initial_content.as_deref().unwrap_or("");
            let mut file = std::fs::File::create(&path).expect("Failed to create temp file");
            file.write_all(content.as_bytes())
                .expect("Failed to write temp file");
            path
        };

        let mut client = self.connect_with_retry().await.expect("Failed to connect");

        // Open buffer file using key sequence
        client
            .send_keys(&format!(":e {temp_path}<CR>"))
            .await
            .expect("Failed to open buffer file");
        tokio::time::sleep(Duration::from_millis(50)).await;

        // Set initial cursor if specified
        if let Some((line, col)) = self.initial_cursor {
            if line > 0 {
                client
                    .send_keys(&format!("{line}j"))
                    .await
                    .expect("Failed to move cursor down");
            }
            if col > 0 {
                client
                    .send_keys(&format!("{col}l"))
                    .await
                    .expect("Failed to move cursor right");
            }
        }

        // Capture initial state
        let initial = self.capture_state(&mut client, "(initial)").await;

        let mut snapshots = Vec::new();
        let mut failed_expectations: Vec<String> = Vec::new();

        // Execute each step
        for step in &self.steps {
            // Send key
            client
                .send_keys(&step.keys)
                .await
                .expect("Failed to inject key");
            tokio::time::sleep(Duration::from_millis(step.delay_ms)).await;

            // Capture state
            let snapshot = self.capture_state(&mut client, &step.keys).await;

            // Verify expectations
            for expectation in &step.expectations {
                match expectation {
                    StepExpectation::Cursor(line, col) => {
                        if snapshot.cursor_line != *line || snapshot.cursor_column != *col {
                            failed_expectations.push(format!(
                                "After '{}': cursor expected ({}:{}), got ({}:{})",
                                step.keys, line, col, snapshot.cursor_line, snapshot.cursor_column
                            ));
                        }
                    }
                    StepExpectation::Buffer(expected) => {
                        if snapshot.buffer.trim_end() != expected.trim_end() {
                            failed_expectations.push(format!(
                                "After '{}': buffer expected {:?}, got {:?}",
                                step.keys, expected, snapshot.buffer
                            ));
                        }
                    }
                    StepExpectation::BufferContains(substring) => {
                        if !snapshot.buffer.contains(substring) {
                            failed_expectations.push(format!(
                                "After '{}': buffer expected to contain {:?}, got {:?}",
                                step.keys, substring, snapshot.buffer
                            ));
                        }
                    }
                    StepExpectation::ModeContains(substring) => {
                        let mode_lower = snapshot.mode_display.to_lowercase();
                        let edit_lower = snapshot.edit_mode.to_lowercase();
                        let sub_lower = substring.to_lowercase();
                        if !mode_lower.contains(&sub_lower) && !edit_lower.contains(&sub_lower) {
                            failed_expectations.push(format!(
                                "After '{}': mode expected to contain {:?}, got display={:?} edit={:?}",
                                step.keys, substring, snapshot.mode_display, snapshot.edit_mode
                            ));
                        }
                    }
                    StepExpectation::EditMode(mode) => {
                        if snapshot.edit_mode != *mode {
                            failed_expectations.push(format!(
                                "After '{}': edit_mode expected {:?}, got {:?}",
                                step.keys, mode, snapshot.edit_mode
                            ));
                        }
                    }
                    StepExpectation::Register(reg, content, yank_type) => {
                        if let Some(register) = snapshot.registers.get(reg) {
                            if register.content.trim_end() != content.trim_end() {
                                failed_expectations.push(format!(
                                    "After '{}': register '{}' content expected {:?}, got {:?}",
                                    step.keys, reg, content, register.content
                                ));
                            }
                            if register.yank_type != *yank_type {
                                failed_expectations.push(format!(
                                    "After '{}': register '{}' type expected {:?}, got {:?}",
                                    step.keys, reg, yank_type, register.yank_type
                                ));
                            }
                        } else {
                            failed_expectations.push(format!(
                                "After '{}': register '{}' not found (available: {:?})",
                                step.keys,
                                reg,
                                snapshot.registers.keys().collect::<Vec<_>>()
                            ));
                        }
                    }
                }
            }

            snapshots.push(snapshot);
        }
        drop(client); // Drop client early to avoid significant_drop_tightening warning

        StepTrace {
            initial,
            snapshots,
            failed_expectations,
            harness: self.harness,
            temp_path: Some(temp_path),
        }
    }
}

/// Trace of step-by-step execution.
pub struct StepTrace {
    /// Initial state before any keys.
    pub initial: StateSnapshot,
    /// State snapshots after each key.
    pub snapshots: Vec<StateSnapshot>,
    /// Failed expectations (if any).
    pub failed_expectations: Vec<String>,
    harness: TestServerHarness,
    temp_path: Option<String>,
}

impl Drop for StepTrace {
    fn drop(&mut self) {
        if let Some(path) = &self.temp_path {
            let _ = std::fs::remove_file(path);
        }
    }
}

impl StepTrace {
    /// Get the path to the server log file for debugging.
    ///
    /// Returns `None` if log capture is not enabled.
    #[must_use]
    pub fn log_path(&self) -> Option<&std::path::Path> {
        self.harness.log_path()
    }

    /// Format log path hint for output messages.
    fn log_hint(&self) -> String {
        self.log_path()
            .map(|p| format!("\n\n  Server log: {}", p.display()))
            .unwrap_or_default()
    }

    /// Print the full trace to stderr (for debugging).
    pub fn print_trace(&self) {
        eprintln!("\n╔════════════════════════════════════════════════════════════╗");
        eprintln!("║                     STEP-BY-STEP TRACE                     ║");
        eprintln!("╚════════════════════════════════════════════════════════════╝\n");

        eprintln!(
            "Initial: {} mode={} ({}:{})",
            self.initial.key,
            self.initial.mode_display,
            self.initial.cursor_line,
            self.initial.cursor_column
        );
        eprintln!("  Buffer: {:?}", self.initial.buffer);

        for (i, snapshot) in self.snapshots.iter().enumerate() {
            eprintln!(
                "\nStep {}: '{}' -> {} ({}:{})",
                i + 1,
                snapshot.key,
                snapshot.mode_display,
                snapshot.cursor_line,
                snapshot.cursor_column
            );
            eprintln!("  Buffer: {:?}", snapshot.buffer);
            if !snapshot.registers.is_empty() {
                eprintln!("  Registers:");
                for (name, info) in &snapshot.registers {
                    eprintln!("    '{}': {:?} ({})", name, info.content, info.yank_type);
                }
            }
        }

        eprintln!("\n────────────────────────────────────────────────────────────");

        if self.failed_expectations.is_empty() {
            eprintln!("OK: All expectations passed");
        } else {
            eprintln!("FAIL: {} expectation(s) failed:", self.failed_expectations.len());
            for failure in &self.failed_expectations {
                eprintln!("  - {failure}");
            }
        }

        // Always show log path at the end for debugging
        if let Some(path) = self.log_path() {
            eprintln!("\n  Server log: {}", path.display());
        }
        eprintln!();
    }

    /// Print compact single-line trace.
    pub fn print_compact(&self) {
        let line: String = std::iter::once(format!(
            "[init {}:{}]",
            self.initial.cursor_line, self.initial.cursor_column
        ))
        .chain(
            self.snapshots
                .iter()
                .map(|s| format!("'{}' -> {}:{}", s.key, s.cursor_line, s.cursor_column)),
        )
        .collect::<Vec<_>>()
        .join(" → ");
        eprintln!("Trace: {line}");
    }

    /// Assert no expectations failed.
    ///
    /// # Panics
    ///
    /// Panics with detailed trace if any expectations failed.
    pub fn assert_ok(&self) {
        if !self.failed_expectations.is_empty() {
            self.print_trace();
            panic!(
                "Step test failed with {} errors:\n{}{}",
                self.failed_expectations.len(),
                self.failed_expectations.join("\n"),
                self.log_hint()
            );
        }
    }

    /// Get final state (last snapshot).
    #[must_use]
    pub fn final_state(&self) -> &StateSnapshot {
        self.snapshots.last().unwrap_or(&self.initial)
    }

    /// Get state after step N (0-indexed).
    #[must_use]
    pub fn state_after(&self, step: usize) -> Option<&StateSnapshot> {
        self.snapshots.get(step)
    }
}
