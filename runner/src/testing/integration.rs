//! Fluent test builder for single-client integration tests.
//!
//! Provides a builder pattern for setting up test scenarios,
//! running key sequences, and asserting results.

// Test infrastructure - suppress pedantic docs requirements
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::missing_panics_doc)]

use std::{
    collections::HashMap,
    io::Write,
    sync::atomic::{AtomicU32, Ordering},
    time::Duration,
};

use serde_json::json;

use crate::client::common::{ConnectionConfig, RpcClient};

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
    config: ConnectionConfig,
    initial_content: Option<String>,
    initial_cursor: Option<(u16, u16)>,
    key_sequences: Vec<KeySequence>,
    default_delay: u64,
}

impl IntegrationTest {
    /// Create new test (spawns server).
    ///
    /// # Panics
    ///
    /// Panics if server fails to spawn.
    pub async fn new() -> Self {
        let harness = TestServerHarness::spawn()
            .await
            .expect("Failed to spawn server");
        let config = ConnectionConfig::tcp("127.0.0.1", harness.port());
        Self {
            harness,
            config,
            initial_content: None,
            initial_cursor: None,
            key_sequences: Vec::new(),
            default_delay: DEFAULT_DELAY_MS,
        }
    }

    /// Make an RPC call with a fresh connection (avoids stale notification issues).
    async fn call(
        &self,
        method: &str,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        let mut client = Self::connect_with_retry(&self.config).await?;
        client.call(method, params).await.map_err(|e| e.to_string())
    }

    /// Connect to server with retry logic.
    async fn connect_with_retry(config: &ConnectionConfig) -> Result<RpcClient, String> {
        let mut attempts = 0;
        loop {
            match RpcClient::connect(config).await {
                Ok(c) => return Ok(c),
                Err(e) if attempts < MAX_CONNECT_ATTEMPTS => {
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
    /// Panics if any RPC call fails.
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
        self.call("buffer/open_file", json!({ "path": &temp_path }))
            .await
            .expect("Failed to open buffer file");

        // Set initial cursor if specified
        if let Some((line, col)) = self.initial_cursor {
            if line > 0 {
                self.call("input/keys", json!({ "keys": format!("{}j", line) }))
                    .await
                    .expect("Failed to move cursor down");
            }
            if col > 0 {
                self.call("input/keys", json!({ "keys": format!("{}l", col) }))
                    .await
                    .expect("Failed to move cursor right");
            }
        }

        // Inject keys
        for seq in &self.key_sequences {
            self.call("input/keys", json!({ "keys": seq.keys }))
                .await
                .expect("Failed to inject keys");
            tokio::time::sleep(Duration::from_millis(seq.delay_ms)).await;
        }

        // Gather results
        let buffer = self
            .call("buffer/get_content", json!({}))
            .await
            .expect("Failed to get buffer content");
        let cursor = self
            .call("state/cursor", json!({}))
            .await
            .expect("Failed to get cursor");
        let mode = self
            .call("state/mode", json!({}))
            .await
            .expect("Failed to get mode");

        // Query registers via debug endpoint
        let registers_result = self
            .call("debug/registers", json!({}))
            .await
            .unwrap_or_else(|_| json!({ "unnamed": {}, "named": [] }));

        // Parse registers into HashMap
        let mut registers = HashMap::new();

        // Parse unnamed register
        if let Some(unnamed) = registers_result.get("unnamed")
            && let (Some(name), Some(content), Some(yank_type)) = (
                unnamed.get("name").and_then(|n| n.as_str()),
                unnamed.get("content").and_then(|c| c.as_str()),
                unnamed.get("yank_type").and_then(|t| t.as_str()),
            )
        {
            registers.insert(
                name.to_string(),
                RegisterInfo {
                    content: content.to_string(),
                    yank_type: yank_type.to_string(),
                },
            );
        }

        // Parse named registers array
        if let Some(named) = registers_result.get("named").and_then(|n| n.as_array()) {
            for entry in named {
                if let (Some(name), Some(content), Some(yank_type)) = (
                    entry.get("name").and_then(|n| n.as_str()),
                    entry.get("content").and_then(|c| c.as_str()),
                    entry.get("yank_type").and_then(|t| t.as_str()),
                ) {
                    registers.insert(
                        name.to_string(),
                        RegisterInfo {
                            content: content.to_string(),
                            yank_type: yank_type.to_string(),
                        },
                    );
                }
            }
        }

        #[allow(clippy::cast_possible_truncation)]
        TestResult {
            buffer_content: buffer["content"].as_str().unwrap_or("").to_string(),
            cursor_line: cursor["line"].as_u64().unwrap_or(0) as u16,
            cursor_column: cursor["column"].as_u64().unwrap_or(0) as u16,
            mode_display: mode["display"].as_str().unwrap_or("").to_string(),
            edit_mode: mode["edit_mode"].as_str().unwrap_or("").to_string(),
            registers,
            _harness: self.harness,
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
    _harness: TestServerHarness,
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
    /// Assert buffer equals expected (trimmed).
    pub fn assert_buffer_eq(&self, expected: &str) {
        assert_eq!(
            self.buffer_content.trim_end(),
            expected.trim_end(),
            "Buffer content mismatch\nExpected:\n{}\nActual:\n{}",
            expected,
            self.buffer_content
        );
    }

    /// Assert buffer contains substring.
    pub fn assert_buffer_contains(&self, expected: &str) {
        assert!(
            self.buffer_content.contains(expected),
            "Buffer does not contain '{}'\nActual:\n{}",
            expected,
            self.buffer_content
        );
    }

    /// Assert cursor position (line, col) - 0-indexed.
    pub fn assert_cursor(&self, line: u16, col: u16) {
        assert_eq!(
            (self.cursor_line, self.cursor_column),
            (line, col),
            "Cursor mismatch: expected (line={}, col={}), got (line={}, col={})",
            line,
            col,
            self.cursor_line,
            self.cursor_column
        );
    }

    /// Assert register content and type.
    pub fn assert_register(&self, reg: &str, expected_content: &str, expected_type: &str) {
        let register = self.registers.get(reg).unwrap_or_else(|| {
            panic!(
                "Register '{}' not found. Available: {:?}",
                reg,
                self.registers.keys().collect::<Vec<_>>()
            )
        });
        assert_eq!(
            register.content.trim_end(),
            expected_content.trim_end(),
            "Register '{}' content mismatch\nExpected: '{}'\nActual: '{}'",
            reg,
            expected_content,
            register.content
        );
        assert_eq!(
            register.yank_type, expected_type,
            "Register '{}' type mismatch\nExpected: '{}'\nActual: '{}'",
            reg, expected_type, register.yank_type
        );
    }

    /// Assert in normal mode.
    pub fn assert_normal_mode(&self) {
        assert!(
            self.edit_mode.to_lowercase().contains("normal")
                || self.mode_display.to_uppercase().contains("NORMAL"),
            "Expected normal mode, got: {} ({})",
            self.mode_display,
            self.edit_mode
        );
    }

    /// Assert in insert mode.
    pub fn assert_insert_mode(&self) {
        assert!(
            self.edit_mode.to_lowercase().contains("insert")
                || self.mode_display.to_uppercase().contains("INSERT"),
            "Expected insert mode, got: {} ({})",
            self.mode_display,
            self.edit_mode
        );
    }

    /// Assert in visual mode.
    pub fn assert_visual_mode(&self) {
        assert!(
            self.edit_mode.to_lowercase().contains("visual")
                || self.mode_display.to_uppercase().contains("VISUAL"),
            "Expected visual mode, got: {} ({})",
            self.mode_display,
            self.edit_mode
        );
    }
}
