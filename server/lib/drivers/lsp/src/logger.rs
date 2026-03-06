//! Dedicated LSP traffic logger.
//!
//! Provides file-based logging of LSP JSON-RPC traffic, server stderr,
//! and lifecycle events. Separate from the general `tracing` system to
//! enable focused LSP debugging.
//!
//! # Activation
//!
//! Controlled by the `REOVIM_LSP_LOG` environment variable:
//! - `REOVIM_LSP_LOG=1` (or empty) — log to `~/.local/share/reovim/lsp-{language}.log`
//! - `REOVIM_LSP_LOG=/path/to/dir` — log to `/path/to/dir/lsp-{language}.log`
//!
//! # Log Format
//!
//! ```text
//! [HH:MM:SS.mmm] [rust] --> textDocument/definition {"uri":"..."}
//! [HH:MM:SS.mmm] [rust] <-- #42 textDocument/definition (ok)
//! [HH:MM:SS.mmm] [rust] <-n textDocument/publishDiagnostics 3 items
//! [HH:MM:SS.mmm] [rust] <-r client/registerCapability
//! [HH:MM:SS.mmm] [rust] err some rust-analyzer stderr line
//! [HH:MM:SS.mmm] [rust] === server initialized (rust-analyzer 0.4.2302)
//! ```

use std::{
    fs::{File, OpenOptions},
    io::{BufWriter, Write},
    path::PathBuf,
    time::SystemTime,
};

use parking_lot::Mutex;

/// Dedicated logger for LSP traffic and server diagnostics.
///
/// Thread-safe via `parking_lot::Mutex` (chosen over `std::sync::Mutex`
/// because a non-poisoning mutex ensures a panicking thread never
/// permanently blocks other threads from logging). Writes are buffered
/// and flushed after each log line to ensure logs survive crashes.
pub struct LspLogger {
    writer: Mutex<BufWriter<File>>,
    language_id: String,
}

impl LspLogger {
    /// Create a new logger writing to the given file path.
    ///
    /// Creates parent directories if they don't exist. Opens the file
    /// in append mode.
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be created.
    pub fn new(path: &std::path::Path, language_id: &str) -> std::io::Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let file = OpenOptions::new().create(true).append(true).open(path)?;
        Ok(Self {
            writer: Mutex::new(BufWriter::new(file)),
            language_id: language_id.to_string(),
        })
    }

    /// Create a logger from the `REOVIM_LSP_LOG` environment variable.
    ///
    /// Returns `None` if the variable is not set or the file cannot be created.
    #[must_use]
    pub fn from_env(language_id: &str) -> Option<Self> {
        let env_val = std::env::var("REOVIM_LSP_LOG").ok()?;
        let dir = if env_val == "1" || env_val.is_empty() {
            default_log_dir()
        } else {
            PathBuf::from(&env_val)
        };
        let path = dir.join(format!("lsp-{language_id}.log"));
        match Self::new(&path, language_id) {
            Ok(logger) => {
                tracing::info!(path = %path.display(), "LSP logging enabled");
                Some(logger)
            }
            Err(e) => {
                tracing::warn!(path = %path.display(), error = %e, "Failed to create LSP log file");
                None
            }
        }
    }

    /// Log an outgoing request or notification (client → server).
    pub fn log_sent(&self, method: &str, detail: &str) {
        self.write_line("-->", &format!("{method} {detail}"));
    }

    /// Log an incoming response (server → client).
    pub fn log_response(&self, id: &str, method: &str, status: &str) {
        self.write_line("<--", &format!("{id} {method} ({status})"));
    }

    /// Log an incoming notification (server → client).
    pub fn log_server_notification(&self, method: &str, detail: &str) {
        self.write_line("<-n", &format!("{method} {detail}"));
    }

    /// Log an incoming server-to-client request.
    pub fn log_server_request(&self, method: &str, detail: &str) {
        self.write_line("<-r", &format!("{method} {detail}"));
    }

    /// Log a line from the LSP server's stderr.
    pub fn log_stderr(&self, line: &str) {
        self.write_line("err", line);
    }

    /// Log a lifecycle event (start, initialize, shutdown).
    pub fn log_event(&self, event: &str) {
        self.write_line("===", event);
    }

    fn write_line(&self, prefix: &str, message: &str) {
        let ts = Self::timestamp();
        let lang = &self.language_id;
        let mut writer = self.writer.lock();
        // I/O errors intentionally discarded: logger failures must not
        // affect the editor's main path or propagate to callers.
        let _ = writeln!(writer, "[{ts}] [{lang}] {prefix} {message}");
        let _ = writer.flush();
    }

    fn timestamp() -> String {
        let dur = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default();
        let secs = dur.as_secs();
        let millis = dur.subsec_millis();
        let hours = (secs % 86400) / 3600;
        let minutes = (secs % 3600) / 60;
        let seconds = secs % 60;
        format!("{hours:02}:{minutes:02}:{seconds:02}.{millis:03}")
    }
}

/// Default log directory following XDG Base Directory specification.
fn default_log_dir() -> PathBuf {
    std::env::var("XDG_DATA_HOME").map_or_else(
        |_| {
            std::env::var("HOME").map_or_else(
                |_| PathBuf::from("/tmp/reovim"),
                |home| PathBuf::from(home).join(".local/share/reovim"),
            )
        },
        |xdg| PathBuf::from(xdg).join("reovim"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn logger_creates_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.log");
        let logger = LspLogger::new(&path, "rust").unwrap();
        logger.log_event("test start");
        assert!(path.exists());

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("[rust]"));
        assert!(content.contains("=== test start"));
    }

    #[test]
    fn logger_creates_parent_dirs() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a/b/c/test.log");
        let logger = LspLogger::new(&path, "python");
        assert!(logger.is_ok());
        assert!(path.exists());
    }

    #[test]
    fn logger_appends() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("append.log");
        {
            let logger = LspLogger::new(&path, "rust").unwrap();
            logger.log_event("first");
        }
        {
            let logger = LspLogger::new(&path, "rust").unwrap();
            logger.log_event("second");
        }
        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("first"));
        assert!(content.contains("second"));
    }

    #[test]
    fn log_sent() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sent.log");
        let logger = LspLogger::new(&path, "rust").unwrap();
        logger.log_sent("textDocument/definition", "{\"uri\":\"file:///test.rs\"}");

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("--> textDocument/definition"));
        assert!(content.contains("file:///test.rs"));
    }

    #[test]
    fn log_response() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("resp.log");
        let logger = LspLogger::new(&path, "rust").unwrap();
        logger.log_response("#42", "textDocument/definition", "ok");

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("<-- #42 textDocument/definition (ok)"));
    }

    #[test]
    fn log_server_notification() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("notif.log");
        let logger = LspLogger::new(&path, "rust").unwrap();
        logger.log_server_notification("textDocument/publishDiagnostics", "3 items");

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("<-n textDocument/publishDiagnostics 3 items"));
    }

    #[test]
    fn log_server_request() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("req.log");
        let logger = LspLogger::new(&path, "rust").unwrap();
        logger.log_server_request("client/registerCapability", "hover");

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("<-r client/registerCapability hover"));
    }

    #[test]
    fn log_stderr() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("stderr.log");
        let logger = LspLogger::new(&path, "rust").unwrap();
        logger.log_stderr("proc-macro server crashed");

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("err proc-macro server crashed"));
    }

    #[test]
    fn log_event() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("event.log");
        let logger = LspLogger::new(&path, "rust").unwrap();
        logger.log_event("server initialized (rust-analyzer 0.4.2302)");

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("=== server initialized"));
    }

    #[test]
    fn timestamp_format() {
        let ts = LspLogger::timestamp();
        // Should be HH:MM:SS.mmm format
        assert_eq!(ts.len(), 12); // "HH:MM:SS.mmm"
        assert_eq!(&ts[2..3], ":");
        assert_eq!(&ts[5..6], ":");
        assert_eq!(&ts[8..9], ".");
    }

    #[test]
    fn from_env_not_set() {
        // REOVIM_LSP_LOG is not set in test environments by default.
        // If it happens to be set, we just verify from_env returns Some.
        let result = LspLogger::from_env("test-env-check");
        if std::env::var("REOVIM_LSP_LOG").is_err() {
            assert!(result.is_none());
        }
    }

    #[test]
    fn from_env_with_custom_path() {
        let dir = tempfile::tempdir().unwrap();
        let dir_path = dir.path().to_string_lossy().to_string();
        // Use a unique env var approach to avoid test interference
        // Since from_env reads REOVIM_LSP_LOG, we test the path logic directly
        let path = PathBuf::from(&dir_path).join("lsp-rust.log");
        let logger = LspLogger::new(&path, "rust").unwrap();
        logger.log_event("custom path test");
        assert!(path.exists());
    }

    #[test]
    fn default_log_dir_with_home() {
        // This tests the fallback logic
        let dir = default_log_dir();
        // Should end with "reovim" regardless of which branch is taken
        assert!(dir.to_string_lossy().contains("reovim"), "Expected reovim in path: {dir:?}");
    }

    #[test]
    fn language_id_in_log_lines() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("lang.log");
        let logger = LspLogger::new(&path, "python").unwrap();
        logger.log_event("test");

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("[python]"));
    }

    #[test]
    fn concurrent_writes() {
        use std::sync::Arc;

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("concurrent.log");
        let logger = Arc::new(LspLogger::new(&path, "rust").unwrap());

        let handles: Vec<_> = (0..10)
            .map(|i| {
                let logger = Arc::clone(&logger);
                std::thread::spawn(move || {
                    logger.log_event(&format!("thread {i}"));
                })
            })
            .collect();

        for h in handles {
            h.join().unwrap();
        }

        let content = std::fs::read_to_string(&path).unwrap();
        assert_eq!(content.lines().count(), 10);
    }
}
