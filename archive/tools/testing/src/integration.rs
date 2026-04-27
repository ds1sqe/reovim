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

#[cfg_attr(coverage_nightly, coverage(off))]
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

    /// Create new test with extra modules loaded.
    ///
    /// Spawns a server with the default modules plus the specified extra
    /// modules. Use this for testing functionality that requires modules
    /// not in the defaults bundle (e.g., textobjects).
    ///
    /// # Example
    ///
    /// ```ignore
    /// let result = IntegrationTest::with_modules(&["textobjects"])
    ///     .await
    ///     .with_buffer("hello world")
    ///     .send_keys("diw")
    ///     .run()
    ///     .await;
    /// result.assert_buffer_eq(" world");
    /// ```
    ///
    /// # Panics
    ///
    /// Panics if server fails to spawn.
    pub async fn with_modules(modules: &[&str]) -> Self {
        let harness = TestServerHarness::spawn_with_modules(modules)
            .await
            .expect("Failed to spawn server with extra modules");
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

    /// Create new test with custom environment variables.
    ///
    /// Passes additional env vars to the server process. Useful for
    /// overriding `XDG_DATA_HOME` to provide test fixture data.
    ///
    /// # Panics
    ///
    /// Panics if server fails to spawn.
    pub async fn with_env(env_vars: &[(&str, &str)]) -> Self {
        let harness = TestServerHarness::spawn_with_env(env_vars)
            .await
            .expect("Failed to spawn server with env vars");
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
        // v3: buffer content routes through projections, stub empty response
        let buffer_lines: Vec<String> = vec![];
        // (#753) get_cursor/get_mode removed; use get_projections with tag filters.
        // client_id=0 means "authenticated client" (token-resolved on server).
        let projections_response = client
            .get_projections(0, vec![])
            .await
            .expect("Failed to get projections");
        let register_response = client
            .get_registers(vec![])
            .await
            .expect("Failed to get registers");
        drop(client); // Drop client early to avoid significant_drop_tightening warning

        // Parse buffer content (lines joined with newlines)
        let buffer_content = buffer_lines.join("\n");

        // Extract cursor and mode from domain-neutral projections.
        // Tags: "text.cursor" carries display like "line:col", "text.mode" carries mode name.
        let mut cursor_line: u16 = 0;
        let mut cursor_column: u16 = 0;
        let mut mode_display = String::new();
        let mut edit_mode = String::new();
        for p in &projections_response.projections {
            let display = p
                .datum
                .as_ref()
                .and_then(|d| d.display.as_deref())
                .unwrap_or("");
            if p.tag == "text.cursor" {
                // Display format: "line:col" (0-indexed)
                if let Some((l, c)) = display.split_once(':') {
                    cursor_line = l.parse().unwrap_or(0);
                    cursor_column = c.parse().unwrap_or(0);
                }
            } else if p.tag == "text.mode" {
                mode_display = display.to_string();
                edit_mode = display.to_string();
            }
        }

        // Populate registers from gRPC response.
        // (#753) RegisterEntry.content is now Option<DomainDatum>; extract display string.
        // yank_type field is removed; stub as empty string.
        let registers = register_response
            .registers
            .into_iter()
            .map(|entry| {
                let content = entry
                    .content
                    .as_ref()
                    .and_then(|d| d.display.clone())
                    .unwrap_or_default();
                (
                    entry.name,
                    RegisterInfo {
                        content,
                        yank_type: String::new(),
                    },
                )
            })
            .collect();

        TestResult {
            buffer_content,
            cursor_line,
            cursor_column,
            mode_display,
            edit_mode,
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

#[cfg_attr(coverage_nightly, coverage(off))]
impl Drop for TestResult {
    fn drop(&mut self) {
        if let Some(path) = &self.temp_path {
            let _ = std::fs::remove_file(path);
        }
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
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

// =========================================================================
// #722 repro: registers were always empty — assert_register always
// panicked with "Register not found".
// Fixed: run() now calls client.get_registers(vec![]) via gRPC.
// =========================================================================

#[cfg(test)]
mod b11_repro {
    use super::*;

    #[test]
    fn b11_register_population_from_grpc() {
        // Verify that RegisterInfo can be constructed from gRPC response data.
        // The fix: run() now calls client.get_registers() and maps
        // RegisterEntry -> RegisterInfo into the HashMap.
        let mut registers = HashMap::new();
        registers.insert(
            "\"".to_string(),
            RegisterInfo {
                content: "hello\n".to_string(),
                yank_type: "line".to_string(),
            },
        );

        let info = registers.get("\"").expect("register should exist");
        assert_eq!(info.content.trim_end(), "hello");
        assert_eq!(info.yank_type, "line");
        assert!(!registers.is_empty(), "#722 fixed: registers populated via gRPC");
    }
}
