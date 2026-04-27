//! TUI-side log buffer for storing and displaying log entries.
//!
//! Provides a ring buffer for log entries from both server and client.

use reovim_protocol::v1::{LogLevel, LogSource};

// ============================================================================
// Constants
// ============================================================================

/// Default TUI log buffer capacity.
pub const DEFAULT_TUI_LOG_CAPACITY: usize = 2000;

// ============================================================================
// TUI Log Entry
// ============================================================================

/// A log entry for display in the TUI.
#[derive(Debug, Clone)]
pub struct TuiLogEntry {
    /// Timestamp in HH:MM:SS format for display.
    pub timestamp: String,
    /// Log level.
    pub level: LogLevel,
    /// Target module.
    pub target: String,
    /// Log message.
    pub message: String,
    /// Source of the log entry.
    pub source: LogSource,
}

impl TuiLogEntry {
    /// Create a new TUI log entry.
    #[must_use]
    pub const fn new(
        timestamp: String,
        level: LogLevel,
        target: String,
        message: String,
        source: LogSource,
    ) -> Self {
        Self {
            timestamp,
            level,
            target,
            message,
            source,
        }
    }

    /// Create a client-side log entry with the current time.
    #[must_use]
    pub fn client_log(level: LogLevel, message: &str) -> Self {
        Self {
            timestamp: current_time_hms(),
            level,
            target: "tui".to_string(),
            message: message.to_string(),
            source: LogSource::Client,
        }
    }

    /// Get the color style for this log level.
    #[must_use]
    pub const fn level_color(&self) -> LevelColor {
        match self.level {
            LogLevel::Error => LevelColor::Red,
            LogLevel::Warn => LevelColor::Yellow,
            LogLevel::Info => LevelColor::Default,
            LogLevel::Debug | LogLevel::Trace => LevelColor::Dim,
        }
    }

    /// Get the source prefix for display.
    #[must_use]
    pub const fn source_prefix(&self) -> &'static str {
        match self.source {
            LogSource::Server => "[S]",
            LogSource::Client => "[C]",
        }
    }
}

/// Color for log level display.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LevelColor {
    /// Red for errors.
    Red,
    /// Yellow for warnings.
    Yellow,
    /// Default terminal color.
    Default,
    /// Dim/gray for debug/trace.
    Dim,
}

impl LevelColor {
    /// Get the ANSI color code.
    #[must_use]
    pub const fn ansi_code(&self) -> &'static str {
        match self {
            Self::Red => "\x1b[31m",
            Self::Yellow => "\x1b[33m",
            Self::Default => "\x1b[0m",
            Self::Dim => "\x1b[90m",
        }
    }

    /// Get the ANSI reset code.
    #[must_use]
    pub const fn reset_code() -> &'static str {
        "\x1b[0m"
    }
}

// ============================================================================
// TUI Log Buffer
// ============================================================================

/// Ring buffer for TUI log entries.
#[derive(Debug)]
pub struct TuiLogBuffer {
    /// Buffer storage.
    entries: Vec<TuiLogEntry>,
    /// Maximum capacity.
    capacity: usize,
    /// Write position (wraps around).
    write_pos: usize,
    /// Total entries written.
    total_written: u64,
}

impl TuiLogBuffer {
    /// Create a new TUI log buffer with the specified capacity.
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: Vec::with_capacity(capacity),
            capacity,
            write_pos: 0,
            total_written: 0,
        }
    }

    /// Push a new entry into the buffer.
    pub fn push(&mut self, entry: TuiLogEntry) {
        if self.entries.len() < self.capacity {
            self.entries.push(entry);
        } else {
            self.entries[self.write_pos] = entry;
        }
        self.write_pos = (self.write_pos + 1) % self.capacity;
        self.total_written += 1;
    }

    /// Get all entries in order (oldest to newest).
    #[must_use]
    pub fn entries(&self) -> Vec<&TuiLogEntry> {
        let len = self.entries.len();
        if len < self.capacity || self.total_written <= self.capacity as u64 {
            // Buffer not full yet, entries are in order
            self.entries.iter().collect()
        } else {
            // Buffer wrapped, need to reorder
            let mut result = Vec::with_capacity(len);
            for i in 0..len {
                let idx = (self.write_pos + i) % len;
                result.push(&self.entries[idx]);
            }
            result
        }
    }

    /// Get the last N entries (newest first).
    #[must_use]
    #[allow(clippy::cast_possible_truncation)]
    #[cfg_attr(coverage_nightly, coverage(off))]
    pub fn tail(&self, n: usize) -> Vec<&TuiLogEntry> {
        let len = self.entries.len();
        let count = n.min(len);

        if count == 0 {
            return Vec::new();
        }

        let mut result = Vec::with_capacity(count);

        // Read backwards from write position
        for i in 0..count {
            let idx = if self.write_pos > i {
                self.write_pos - 1 - i
            } else if len > 0 {
                len - 1 - (i - self.write_pos)
            } else {
                break;
            };
            result.push(&self.entries[idx]);
        }

        result
    }

    /// Get the number of entries in the buffer.
    #[must_use]
    #[allow(clippy::missing_const_for_fn)] // Vec::len is not const stable
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check if the buffer is empty.
    #[must_use]
    #[allow(clippy::missing_const_for_fn)] // Vec::is_empty is not const stable
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Clear all entries.
    pub fn clear(&mut self) {
        self.entries.clear();
        self.write_pos = 0;
        // Keep total_written for statistics
    }

    /// Get the buffer capacity.
    #[must_use]
    pub const fn capacity(&self) -> usize {
        self.capacity
    }

    /// Get total entries written (for overflow calculation).
    #[must_use]
    pub const fn total_written(&self) -> u64 {
        self.total_written
    }
}

impl Default for TuiLogBuffer {
    fn default() -> Self {
        Self::new(DEFAULT_TUI_LOG_CAPACITY)
    }
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Get the current time in HH:MM:SS format.
fn current_time_hms() -> String {
    use std::time::SystemTime;

    let now = SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();

    let secs = now.as_secs();
    let time_of_day = secs % 86400;
    let hours = time_of_day / 3600;
    let minutes = (time_of_day % 3600) / 60;
    let seconds = time_of_day % 60;

    format!("{hours:02}:{minutes:02}:{seconds:02}")
}

#[cfg(test)]
#[path = "log_buffer_tests.rs"]
mod tests;
