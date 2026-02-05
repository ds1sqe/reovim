//! Fluent test builder for single-client integration tests.
//!
//! Provides a builder pattern for setting up test scenarios,
//! running key sequences, and asserting results.
//!
//! # Protocol
//!
//! This module uses **gRPC v2** for communication with the server.
//! The legacy JSON-RPC v1 protocol is no longer supported.

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

use super::harness::TestServerHarness;

/// Connection retry settings
const MAX_CONNECT_ATTEMPTS: u32 = 20;
const RETRY_BASE_MS: u64 = 50;
const RETRY_MAX_MS: u64 = 200;

/// Default delay between key sequences (allows event processing)
const DEFAULT_DELAY_MS: u64 = 50;

/// Counter for unique temp file names
static TEMP_FILE_COUNTER: AtomicU32 = AtomicU32::new(0);

/// Key sequence with optional delay
struct KeySequence {
    keys: String,
    delay_ms: u64,
}

/// Integration test builder.
///
/// # Example
///
/// ```ignore
/// let result = IntegrationTest::new()
///     .await
///     .with_buffer("hello world")
///     .send_keys("dw")
///     .run()
///     .await;
/// result.assert_buffer_eq("world");
/// ```
pub struct IntegrationTest {
    harness: TestServerHarness,
    addr: String,
    initial_content: Option<String>,
    initial_cursor: Option<(u16, u16)>,
    key_sequences: Vec<KeySequence>,
    default_delay: u64,
}

impl IntegrationTest {
    /// Create new test with automatic log capture (spawns server).
    ///
    /// Server logs are captured to `tmp/test-logs/{test_name}_{timestamp}.log`.
    /// This is invaluable for debugging test failures.
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
            key_sequences: Vec::new(),
            default_delay: DEFAULT_DELAY_MS,
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

    /// Set initial buffer content.
    #[must_use]
    pub fn with_buffer(mut self, content: &str) -> Self {
        self.initial_content = Some(content.to_string());
        self
    }

    /// Load initial content from file.
    ///
    /// # Panics
    ///
    /// Panics if file cannot be read.
    #[must_use]
    pub fn with_file(mut self, path: &str) -> Self {
        let content = std::fs::read_to_string(path)
            .unwrap_or_else(|e| panic!("Failed to read file '{path}': {e}"));
        self.initial_content = Some(content);
        self
    }

    /// Set initial cursor position (line, col) - 0-indexed.
    #[must_use]
    #[allow(clippy::missing_const_for_fn)]
    pub fn with_cursor_at(mut self, line: u16, col: u16) -> Self {
        self.initial_cursor = Some((line, col));
        self
    }

    /// Add key sequence to send.
    #[must_use]
    pub fn send_keys(mut self, keys: &str) -> Self {
        self.key_sequences.push(KeySequence {
            keys: keys.to_string(),
            delay_ms: self.default_delay,
        });
        self
    }

    /// Add delay after last key sequence.
    #[must_use]
    pub fn with_delay(mut self, ms: u64) -> Self {
        if let Some(last) = self.key_sequences.last_mut() {
            last.delay_ms = ms;
        }
        self
    }

    /// Run test and return result.
    ///
    /// # Panics
    ///
    /// Panics if any gRPC call fails.
    #[allow(clippy::too_many_lines)]
    pub async fn run(self) -> TestResult {
        // Create buffer via temp file
        let temp_path = {
            let id = TEMP_FILE_COUNTER.fetch_add(1, Ordering::SeqCst);
            let path = format!("/tmp/reovim-test-{}-{id}.txt", std::process::id());
            let content = self.initial_content.as_deref().unwrap_or("");
            let mut file = std::fs::File::create(&path).expect("Failed to create temp file");
            file.write_all(content.as_bytes())
                .expect("Failed to write temp file");
            path
        };

        // Connect and set up buffer
        let mut client = self.connect_with_retry().await.expect("Failed to connect");

        // Open buffer file using key sequence (since we don't have a direct file open API yet)
        // TODO: Add file open API to gRPC services
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

        // Inject keys
        for seq in &self.key_sequences {
            client
                .send_keys(&seq.keys)
                .await
                .expect("Failed to inject keys");
            tokio::time::sleep(Duration::from_millis(seq.delay_ms)).await;
        }

        // Gather results using gRPC
        let buffer_response = client
            .get_buffer_content(None)
            .await
            .expect("Failed to get buffer content");
        let cursor_response = client.get_cursor().await.expect("Failed to get cursor");
        let mode_response = client.get_mode().await.expect("Failed to get mode");
        drop(client); // Drop client early to avoid significant_drop_tightening warning

        // Parse buffer content (lines joined with newlines)
        let buffer_content = buffer_response.lines.join("\n");

        // Extract cursor position from nested Position message
        let (cursor_line, cursor_column) = cursor_response
            .position
            .map_or((0, 0), |pos| (pos.line, pos.column));

        // TODO: Implement register query via gRPC (Phase 9+)
        // For now, registers are empty
        let registers = HashMap::new();

        #[allow(clippy::cast_possible_truncation)]
        TestResult {
            buffer_content,
            cursor_line: cursor_line as u16,
            cursor_column: cursor_column as u16,
            mode_display: mode_response.display,
            edit_mode: mode_response.name,
            registers,
            harness: self.harness,
            temp_path: Some(temp_path),
        }
    }
}

/// Register information from debug endpoint.
#[derive(Debug, Clone)]
pub struct RegisterInfo {
    /// Register content.
    pub content: String,
    /// Yank type: "linewise" or "characterwise".
    pub yank_type: String,
}

/// Test result with assertion methods.
pub struct TestResult {
    /// Buffer content after test.
    pub buffer_content: String,
    /// Cursor line (0-indexed).
    pub cursor_line: u16,
    /// Cursor column (0-indexed).
    pub cursor_column: u16,
    /// Mode display name (e.g., "NORMAL").
    pub mode_display: String,
    /// Edit mode string.
    pub edit_mode: String,
    /// Register contents.
    pub registers: HashMap<String, RegisterInfo>,
    harness: TestServerHarness,
    temp_path: Option<String>,
}

impl Drop for TestResult {
    fn drop(&mut self) {
        if let Some(path) = &self.temp_path {
            let _ = std::fs::remove_file(path);
        }
    }
}

impl TestResult {
    /// Get the path to the server log file for debugging.
    ///
    /// Returns `None` if log capture is not enabled.
    #[must_use]
    pub fn log_path(&self) -> Option<&std::path::Path> {
        self.harness.log_path()
    }

    /// Format log path hint for assertion messages.
    fn log_hint(&self) -> String {
        self.log_path()
            .map(|p| format!("\n\nServer log: {}", p.display()))
            .unwrap_or_default()
    }

    /// Assert buffer equals expected (trimmed).
    pub fn assert_buffer_eq(&self, expected: &str) {
        assert!(
            self.buffer_content.trim_end() == expected.trim_end(),
            "assertion `left == right` failed: Buffer content mismatch\n\
             Expected:\n{}\n\
             Actual:\n{}{}",
            expected,
            self.buffer_content,
            self.log_hint()
        );
    }

    /// Assert buffer contains substring.
    pub fn assert_buffer_contains(&self, expected: &str) {
        assert!(
            self.buffer_content.contains(expected),
            "Buffer does not contain '{}'\nActual:\n{}{}",
            expected,
            self.buffer_content,
            self.log_hint()
        );
    }

    /// Assert cursor position (line, col) - 0-indexed.
    pub fn assert_cursor(&self, line: u16, col: u16) {
        assert!(
            (self.cursor_line, self.cursor_column) == (line, col),
            "Cursor mismatch: expected (line={}, col={}), got (line={}, col={}){}",
            line,
            col,
            self.cursor_line,
            self.cursor_column,
            self.log_hint()
        );
    }

    /// Assert register content and type.
    pub fn assert_register(&self, reg: &str, expected_content: &str, expected_type: &str) {
        let register = self.registers.get(reg).unwrap_or_else(|| {
            panic!(
                "Register '{}' not found. Available: {:?}{}",
                reg,
                self.registers.keys().collect::<Vec<_>>(),
                self.log_hint()
            )
        });
        assert!(
            register.content.trim_end() == expected_content.trim_end(),
            "Register '{}' content mismatch\nExpected: '{}'\nActual: '{}'{}",
            reg,
            expected_content,
            register.content,
            self.log_hint()
        );
        assert!(
            register.yank_type == expected_type,
            "Register '{}' type mismatch\nExpected: '{}'\nActual: '{}'{}",
            reg,
            expected_type,
            register.yank_type,
            self.log_hint()
        );
    }

    /// Assert in normal mode.
    pub fn assert_normal_mode(&self) {
        if !self.edit_mode.to_lowercase().contains("normal")
            && !self.mode_display.to_uppercase().contains("NORMAL")
        {
            panic!(
                "Expected normal mode, got: {} ({}){}",
                self.mode_display,
                self.edit_mode,
                self.log_hint()
            );
        }
    }

    /// Assert in insert mode.
    pub fn assert_insert_mode(&self) {
        if !self.edit_mode.to_lowercase().contains("insert")
            && !self.mode_display.to_uppercase().contains("INSERT")
        {
            panic!(
                "Expected insert mode, got: {} ({}){}",
                self.mode_display,
                self.edit_mode,
                self.log_hint()
            );
        }
    }

    /// Assert in visual mode.
    pub fn assert_visual_mode(&self) {
        if !self.edit_mode.to_lowercase().contains("visual")
            && !self.mode_display.to_uppercase().contains("VISUAL")
        {
            panic!(
                "Expected visual mode, got: {} ({}){}",
                self.mode_display,
                self.edit_mode,
                self.log_hint()
            );
        }
    }
}
