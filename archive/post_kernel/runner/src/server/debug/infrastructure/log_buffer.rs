//! Log ring buffer for capturing recent log entries.
//!
//! Provides a thread-safe circular buffer for storing recent log entries
//! accessible via the debug API.

use std::{
    sync::{
        OnceLock,
        atomic::{AtomicU64, AtomicUsize, Ordering},
    },
    time::SystemTime,
};

use reovim_arch::sync::RwLock;

// ============================================================================
// Constants
// ============================================================================

/// Default log buffer capacity.
pub const DEFAULT_LOG_BUFFER_CAPACITY: usize = 1000;

// ============================================================================
// Global Log Buffer
// ============================================================================

/// Global log buffer.
static LOG_BUFFER: OnceLock<LogRingBuffer> = OnceLock::new();

/// Get or initialize the global log buffer.
pub fn log_buffer() -> &'static LogRingBuffer {
    LOG_BUFFER.get_or_init(|| LogRingBuffer::new(DEFAULT_LOG_BUFFER_CAPACITY))
}

/// Get current log level as string.
#[must_use]
pub fn current_log_level() -> String {
    // Read from tracing subscriber max level
    tracing::level_filters::STATIC_MAX_LEVEL.to_string()
}

// ============================================================================
// Log Entry
// ============================================================================

/// A single log entry.
#[derive(Debug, Clone)]
pub struct LogEntry {
    /// Timestamp when the entry was created.
    pub timestamp: SystemTime,
    /// Log level.
    pub level: String,
    /// Target module.
    pub target: String,
    /// Log message.
    pub message: String,
}

impl LogEntry {
    /// Create a new log entry.
    #[must_use]
    pub fn new(level: &str, target: &str, message: &str) -> Self {
        Self {
            timestamp: SystemTime::now(),
            level: level.to_string(),
            target: target.to_string(),
            message: message.to_string(),
        }
    }

    /// Format timestamp as ISO 8601.
    #[must_use]
    pub fn timestamp_iso(&self) -> String {
        // Simple ISO 8601 formatting without chrono
        let duration = self
            .timestamp
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default();

        let secs = duration.as_secs();
        let millis = duration.subsec_millis();

        // Convert to date/time components
        let days_since_epoch = secs / 86400;
        let time_of_day = secs % 86400;

        let hours = time_of_day / 3600;
        let minutes = (time_of_day % 3600) / 60;
        let seconds = time_of_day % 60;

        // Simple year/month/day calculation
        let (year, month, day) = days_to_ymd(days_since_epoch);

        format!("{year:04}-{month:02}-{day:02}T{hours:02}:{minutes:02}:{seconds:02}.{millis:03}Z")
    }
}

/// Convert days since Unix epoch to year/month/day.
#[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
fn days_to_ymd(days: u64) -> (u32, u32, u32) {
    // Days from Unix epoch (1970-01-01)
    let mut remaining_days = days as i64;

    // Start from 1970
    let mut year: i32 = 1970;

    // Find year
    loop {
        let days_in_year: i64 = if is_leap_year(year) { 366 } else { 365 };
        if remaining_days < days_in_year {
            break;
        }
        remaining_days -= days_in_year;
        year += 1;
    }

    // Find month
    let is_leap = is_leap_year(year);
    let month_days = if is_leap {
        [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };

    let mut month: usize = 0;
    for (i, &days) in month_days.iter().enumerate() {
        if remaining_days < i64::from(days) {
            month = i;
            break;
        }
        remaining_days -= i64::from(days);
    }

    #[allow(clippy::cast_sign_loss)]
    (year as u32, (month + 1) as u32, remaining_days as u32 + 1)
}

/// Check if year is a leap year.
#[allow(clippy::manual_is_multiple_of)]
const fn is_leap_year(year: i32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

// ============================================================================
// Log Ring Buffer
// ============================================================================

/// Thread-safe circular buffer for log entries.
#[derive(Debug)]
pub struct LogRingBuffer {
    /// Buffer storage.
    entries: RwLock<Vec<Option<LogEntry>>>,
    /// Buffer capacity.
    capacity: usize,
    /// Write position (wraps around).
    write_pos: AtomicUsize,
    /// Total entries written (for overflow calculation).
    total_written: AtomicU64,
}

impl LogRingBuffer {
    /// Create a new log ring buffer.
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        let mut entries = Vec::with_capacity(capacity);
        entries.resize_with(capacity, || None);

        Self {
            entries: RwLock::new(entries),
            capacity,
            write_pos: AtomicUsize::new(0),
            total_written: AtomicU64::new(0),
        }
    }

    /// Push a new entry into the buffer.
    pub fn push(&self, entry: LogEntry) {
        let pos = self.write_pos.fetch_add(1, Ordering::SeqCst) % self.capacity;
        self.total_written.fetch_add(1, Ordering::SeqCst);

        let mut guard = self.entries.write();
        guard[pos] = Some(entry);
    }

    /// Get the last N entries (newest first).
    #[must_use]
    #[allow(clippy::cast_possible_truncation)]
    pub fn tail(&self, n: usize) -> Vec<LogEntry> {
        let guard = self.entries.read();
        let total = self.total_written.load(Ordering::SeqCst);
        let write_pos = self.write_pos.load(Ordering::SeqCst);

        // Calculate how many entries we actually have
        let count = std::cmp::min(n, std::cmp::min(total as usize, self.capacity));

        if count == 0 {
            return Vec::new();
        }

        let mut result = Vec::with_capacity(count);

        // Read backwards from write position
        for i in 0..count {
            let pos = if write_pos > i {
                write_pos - 1 - i
            } else {
                self.capacity - 1 - (i - write_pos)
            };

            if let Some(entry) = &guard[pos % self.capacity] {
                result.push(entry.clone());
            }
        }

        result
    }

    /// Get total entries written (for overflow calculation).
    #[must_use]
    pub fn total_written(&self) -> u64 {
        self.total_written.load(Ordering::SeqCst)
    }

    /// Get overflow count (entries that were overwritten).
    #[must_use]
    pub fn overflow_count(&self) -> u64 {
        let total = self.total_written.load(Ordering::SeqCst);
        total.saturating_sub(self.capacity as u64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_log_entry_new() {
        let entry = LogEntry::new("INFO", "test::module", "test message");
        assert_eq!(entry.level, "INFO");
        assert_eq!(entry.target, "test::module");
        assert_eq!(entry.message, "test message");
    }

    #[test]
    fn test_log_entry_timestamp_iso() {
        let entry = LogEntry::new("INFO", "test", "msg");
        let iso = entry.timestamp_iso();
        // Should be a valid ISO 8601 format
        assert!(iso.contains('T'));
        assert!(iso.ends_with('Z'));
    }

    #[test]
    fn test_log_ring_buffer_new() {
        let buffer = LogRingBuffer::new(10);
        assert_eq!(buffer.total_written(), 0);
        assert_eq!(buffer.overflow_count(), 0);
        assert!(buffer.tail(10).is_empty());
    }

    #[test]
    fn test_log_ring_buffer_push_and_tail() {
        let buffer = LogRingBuffer::new(10);

        buffer.push(LogEntry::new("INFO", "test", "msg1"));
        buffer.push(LogEntry::new("DEBUG", "test", "msg2"));
        buffer.push(LogEntry::new("WARN", "test", "msg3"));

        let entries = buffer.tail(10);
        assert_eq!(entries.len(), 3);
        // Newest first
        assert_eq!(entries[0].message, "msg3");
        assert_eq!(entries[1].message, "msg2");
        assert_eq!(entries[2].message, "msg1");
    }

    #[test]
    fn test_log_ring_buffer_overflow() {
        let buffer = LogRingBuffer::new(3);

        for i in 0..5 {
            buffer.push(LogEntry::new("INFO", "test", &format!("msg{i}")));
        }

        assert_eq!(buffer.total_written(), 5);
        assert_eq!(buffer.overflow_count(), 2);

        let entries = buffer.tail(10);
        assert_eq!(entries.len(), 3);
        // Should have the last 3 messages
        assert_eq!(entries[0].message, "msg4");
        assert_eq!(entries[1].message, "msg3");
        assert_eq!(entries[2].message, "msg2");
    }

    #[test]
    fn test_log_ring_buffer_tail_limit() {
        let buffer = LogRingBuffer::new(10);

        for i in 0..5 {
            buffer.push(LogEntry::new("INFO", "test", &format!("msg{i}")));
        }

        let entries = buffer.tail(2);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].message, "msg4");
        assert_eq!(entries[1].message, "msg3");
    }

    #[test]
    fn test_days_to_ymd() {
        // 1970-01-01
        assert_eq!(days_to_ymd(0), (1970, 1, 1));
        // 2000-01-01
        assert_eq!(days_to_ymd(10957), (2000, 1, 1));
        // 2025-01-14
        assert_eq!(days_to_ymd(20102), (2025, 1, 14));
    }

    #[test]
    fn test_is_leap_year() {
        assert!(!is_leap_year(1970));
        assert!(is_leap_year(2000));
        assert!(!is_leap_year(2001));
        assert!(is_leap_year(2024));
        assert!(!is_leap_year(1900));
    }
}
