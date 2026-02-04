//! Per-client debug ring buffer for event tracking.
//!
//! This module provides a fixed-capacity ring buffer (8 KB default) that captures
//! client-specific events like key presses, commands, mode changes, and errors.
//!
//! # Architecture
//!
//! ```text
//! Client (Owner)
//! ├── EditingState (mode, cursor, selection)
//! └── ClientRingBuffer (events for this client)
//!     ├── KeyPress events
//!     ├── CommandExecuted events
//!     ├── ModeChanged events
//!     └── Error events
//! ```
//!
//! # Thread Safety
//!
//! Uses `parking_lot::RwLock` for fast, non-poisoning concurrent access.
//!
//! # Usage
//!
//! ```ignore
//! use reovim_server::session::ClientRingBuffer;
//!
//! let buffer = ClientRingBuffer::new();
//! buffer.log_event(ClientEventType::KeyPress, "pressed 'j'");
//! buffer.log_event(ClientEventType::ModeChanged, "normal -> insert");
//!
//! // Get recent events
//! let recent = buffer.tail(10);
//!
//! // Dump for debugging
//! let dump = buffer.dump();
//! ```

use std::{fmt::Write, time::Instant};

use parking_lot::RwLock;

// =============================================================================
// Constants
// =============================================================================

/// Default capacity for client ring buffer (8 KB).
pub const DEFAULT_CLIENT_CAPACITY: usize = 8 * 1024;

/// Maximum event details length before truncation (1 KB).
pub const MAX_DETAILS_LEN: usize = 1024;

// =============================================================================
// Event Types
// =============================================================================

/// Type of client event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClientEventType {
    /// Key press received.
    KeyPress,
    /// Command executed.
    CommandExecuted,
    /// Mode transition.
    ModeChanged,
    /// Cursor/selection/state change.
    StateChanged,
    /// Error encountered.
    Error,
    /// Warning.
    Warning,
    /// Informational message.
    Info,
}

impl ClientEventType {
    /// Returns the string representation.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::KeyPress => "KEY",
            Self::CommandExecuted => "CMD",
            Self::ModeChanged => "MODE",
            Self::StateChanged => "STATE",
            Self::Error => "ERROR",
            Self::Warning => "WARN",
            Self::Info => "INFO",
        }
    }
}

impl std::fmt::Display for ClientEventType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

// =============================================================================
// Log Entry
// =============================================================================

/// A single log entry in the client ring buffer.
#[derive(Debug, Clone)]
pub struct ClientLogEntry {
    /// Per-client sequence number.
    pub seq: u64,
    /// Timestamp in microseconds since client connected.
    pub timestamp_us: u64,
    /// Event type.
    pub event_type: ClientEventType,
    /// Event details (truncated to `MAX_DETAILS_LEN`).
    pub details: String,
}

impl ClientLogEntry {
    /// Estimates the memory size of this entry in bytes.
    #[allow(clippy::missing_const_for_fn)]
    fn size_bytes(&self) -> usize {
        // Fixed fields: seq(8) + timestamp_us(8) + event_type(1) = 17
        // Plus details heap allocation
        17 + self.details.capacity()
    }
}

// =============================================================================
// Buffer Statistics
// =============================================================================

/// Statistics about the client ring buffer.
#[derive(Debug, Clone, Copy)]
pub struct ClientBufferStats {
    /// Maximum capacity in bytes.
    pub capacity_bytes: usize,
    /// Current usage in bytes.
    pub bytes_used: usize,
    /// Number of entries currently in the buffer.
    pub entry_count: usize,
    /// Total number of entries ever logged.
    pub total_logged: u64,
    /// Number of entries dropped due to overflow.
    pub dropped: u64,
}

// =============================================================================
// Ring Buffer Inner
// =============================================================================

/// Inner state of the client ring buffer, protected by `RwLock`.
#[derive(Debug)]
struct ClientRingBufferInner {
    /// Log entries in insertion order.
    entries: Vec<ClientLogEntry>,
    /// Total entries logged (monotonic, never wraps).
    total_logged: u64,
    /// Current byte usage.
    bytes_used: usize,
    /// Maximum capacity in bytes.
    capacity_bytes: usize,
    /// Client connection time for timestamps.
    start_time: Instant,
}

impl ClientRingBufferInner {
    /// Creates a new ring buffer inner with the given capacity.
    fn new(capacity_bytes: usize) -> Self {
        Self {
            entries: Vec::new(),
            total_logged: 0,
            bytes_used: 0,
            capacity_bytes,
            start_time: Instant::now(),
        }
    }

    /// Pushes a new entry, evicting old entries if necessary.
    fn push(&mut self, event_type: ClientEventType, details: String) {
        // Truncate details if too long
        let details = if details.len() > MAX_DETAILS_LEN {
            let mut truncated = details[..MAX_DETAILS_LEN].to_string();
            truncated.push_str("...");
            truncated
        } else {
            details
        };

        let entry = ClientLogEntry {
            seq: self.total_logged,
            #[allow(clippy::cast_possible_truncation)]
            timestamp_us: self.start_time.elapsed().as_micros() as u64,
            event_type,
            details,
        };

        let entry_size = entry.size_bytes();
        self.total_logged += 1;

        // Evict old entries until we have room
        while self.bytes_used + entry_size > self.capacity_bytes && !self.entries.is_empty() {
            let removed = self.entries.remove(0);
            self.bytes_used = self.bytes_used.saturating_sub(removed.size_bytes());
        }

        // Add new entry
        self.entries.push(entry);
        self.bytes_used += entry_size;
    }

    /// Returns the N most recent entries (newest first).
    fn tail(&self, n: usize) -> Vec<ClientLogEntry> {
        let count = n.min(self.entries.len());
        self.entries.iter().rev().take(count).cloned().collect()
    }

    /// Returns all entries in order (oldest to newest).
    fn entries(&self) -> Vec<ClientLogEntry> {
        self.entries.clone()
    }

    /// Formats all entries as a string for crash dumps.
    fn dump(&self) -> String {
        let mut output = String::new();
        output.push_str("=== Client Ring Buffer Dump ===\n");
        let _ = writeln!(
            output,
            "Entries: {} | Bytes: {}/{} | Total logged: {}",
            self.entries.len(),
            self.bytes_used,
            self.capacity_bytes,
            self.total_logged
        );
        output.push_str("---\n");

        for entry in &self.entries {
            let _ = writeln!(
                output,
                "[{:>10}us] {:5} {}",
                entry.timestamp_us,
                entry.event_type.as_str(),
                entry.details
            );
        }

        output.push_str("=== End Dump ===\n");
        output
    }

    /// Returns buffer statistics.
    #[allow(clippy::missing_const_for_fn)]
    fn stats(&self) -> ClientBufferStats {
        let dropped = if self.total_logged > self.entries.len() as u64 {
            self.total_logged - self.entries.len() as u64
        } else {
            0
        };

        ClientBufferStats {
            capacity_bytes: self.capacity_bytes,
            bytes_used: self.bytes_used,
            entry_count: self.entries.len(),
            total_logged: self.total_logged,
            dropped,
        }
    }
}

// =============================================================================
// Client Ring Buffer
// =============================================================================

/// Per-client debug ring buffer.
///
/// A fixed-capacity (8 KB default) ring buffer that captures client-specific
/// events for debugging. When the buffer is full, oldest entries are discarded.
///
/// # Thread Safety
///
/// All operations are thread-safe. Multiple readers can access the buffer
/// simultaneously; writers are exclusive.
pub struct ClientRingBuffer {
    inner: RwLock<ClientRingBufferInner>,
}

impl ClientRingBuffer {
    /// Creates a new ring buffer with default capacity (8 KB).
    #[must_use]
    pub fn new() -> Self {
        Self::with_capacity(DEFAULT_CLIENT_CAPACITY)
    }

    /// Creates a new ring buffer with the specified capacity in bytes.
    #[must_use]
    pub fn with_capacity(capacity_bytes: usize) -> Self {
        Self {
            inner: RwLock::new(ClientRingBufferInner::new(capacity_bytes)),
        }
    }

    /// Logs an event to the buffer.
    ///
    /// If the buffer would exceed capacity, oldest entries are discarded.
    pub fn log_event(&self, event_type: ClientEventType, details: impl Into<String>) {
        // Use try_write to avoid blocking during panic
        if let Some(mut inner) = self.inner.try_write() {
            inner.push(event_type, details.into());
        }
        // If we can't get the lock, skip this entry (panic safety)
    }

    /// Logs a key press event.
    pub fn log_key(&self, key: &str) {
        self.log_event(ClientEventType::KeyPress, key);
    }

    /// Logs a command execution event.
    pub fn log_command(&self, command: &str) {
        self.log_event(ClientEventType::CommandExecuted, command);
    }

    /// Logs a mode change event.
    pub fn log_mode_change(&self, from: &str, to: &str) {
        self.log_event(ClientEventType::ModeChanged, format!("{from} -> {to}"));
    }

    /// Logs a state change event.
    pub fn log_state_change(&self, description: &str) {
        self.log_event(ClientEventType::StateChanged, description);
    }

    /// Logs an error event.
    pub fn log_error(&self, error: &str) {
        self.log_event(ClientEventType::Error, error);
    }

    /// Returns the N most recent entries (newest first).
    #[must_use]
    pub fn tail(&self, n: usize) -> Vec<ClientLogEntry> {
        self.inner.read().tail(n)
    }

    /// Returns all entries in order (oldest to newest).
    #[must_use]
    pub fn entries(&self) -> Vec<ClientLogEntry> {
        self.inner.read().entries()
    }

    /// Formats all entries as a string for crash dumps.
    ///
    /// Blocks until the lock is acquired.
    #[must_use]
    pub fn dump(&self) -> String {
        self.inner.read().dump()
    }

    /// Attempts to format all entries without blocking.
    ///
    /// Returns `None` if the lock cannot be acquired immediately.
    /// Use this in panic handlers to avoid deadlocks.
    #[must_use]
    pub fn try_dump(&self) -> Option<String> {
        self.inner.try_read().map(|inner| inner.dump())
    }

    /// Returns buffer statistics.
    #[must_use]
    pub fn stats(&self) -> ClientBufferStats {
        self.inner.read().stats()
    }

    /// Returns the current byte usage.
    #[must_use]
    pub fn bytes_used(&self) -> usize {
        self.inner.read().bytes_used
    }

    /// Clears all entries from the buffer.
    pub fn clear(&self) {
        if let Some(mut inner) = self.inner.try_write() {
            inner.entries.clear();
            inner.bytes_used = 0;
        }
    }
}

impl Default for ClientRingBuffer {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for ClientRingBuffer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let stats = self.stats();
        f.debug_struct("ClientRingBuffer")
            .field("capacity_bytes", &stats.capacity_bytes)
            .field("bytes_used", &stats.bytes_used)
            .field("entry_count", &stats.entry_count)
            .field("total_logged", &stats.total_logged)
            .finish()
    }
}

impl Clone for ClientRingBuffer {
    fn clone(&self) -> Self {
        let inner = self.inner.read();
        let capacity = inner.capacity_bytes;
        let entries = inner.entries.clone();
        let total_logged = inner.total_logged;
        let bytes_used = inner.bytes_used;
        let start_time = inner.start_time;
        drop(inner);

        let mut new_inner = ClientRingBufferInner::new(capacity);
        new_inner.entries = entries;
        new_inner.total_logged = total_logged;
        new_inner.bytes_used = bytes_used;
        new_inner.start_time = start_time;
        Self {
            inner: RwLock::new(new_inner),
        }
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_log_and_tail() {
        let buffer = ClientRingBuffer::new();

        buffer.log_key("j");
        buffer.log_key("k");
        buffer.log_command("delete");

        let recent = buffer.tail(2);
        assert_eq!(recent.len(), 2);
        assert_eq!(recent[0].event_type, ClientEventType::CommandExecuted);
        assert_eq!(recent[0].details, "delete");
        assert_eq!(recent[1].event_type, ClientEventType::KeyPress);
        assert_eq!(recent[1].details, "k");
    }

    #[test]
    fn test_entries_order() {
        let buffer = ClientRingBuffer::new();

        buffer.log_event(ClientEventType::Info, "first");
        buffer.log_event(ClientEventType::Info, "second");
        buffer.log_event(ClientEventType::Info, "third");

        let all = buffer.entries();
        assert_eq!(all.len(), 3);
        assert_eq!(all[0].details, "first");
        assert_eq!(all[1].details, "second");
        assert_eq!(all[2].details, "third");
    }

    #[test]
    fn test_wrap_around() {
        // Small buffer that will definitely wrap
        let buffer = ClientRingBuffer::with_capacity(100);

        for i in 0..20 {
            buffer.log_event(ClientEventType::Info, format!("event {i}"));
        }

        let stats = buffer.stats();
        assert!(stats.bytes_used <= 100, "Should not exceed capacity");
        assert!(stats.dropped > 0, "Should have dropped entries");
        assert!(stats.total_logged == 20, "Should have logged 20 total");
    }

    #[test]
    fn test_details_truncation() {
        let buffer = ClientRingBuffer::new();

        let long_details = "x".repeat(MAX_DETAILS_LEN + 100);
        buffer.log_event(ClientEventType::Info, long_details);

        let entries = buffer.entries();
        assert_eq!(entries.len(), 1);
        assert!(entries[0].details.len() <= MAX_DETAILS_LEN + 10, "Details should be truncated");
        assert!(entries[0].details.ends_with("..."), "Should have truncation marker");
    }

    #[test]
    fn test_dump_formatting() {
        let buffer = ClientRingBuffer::new();

        buffer.log_error("test error");
        buffer.log_key("j");

        let dump = buffer.dump();
        assert!(dump.contains("Client Ring Buffer Dump"));
        assert!(dump.contains("test error"));
        assert!(dump.contains("ERROR"));
        assert!(dump.contains("KEY"));
    }

    #[test]
    fn test_try_dump() {
        let buffer = ClientRingBuffer::new();
        buffer.log_event(ClientEventType::Info, "test");

        let dump = buffer.try_dump();
        assert!(dump.is_some());
        assert!(dump.unwrap().contains("test"));
    }

    #[test]
    fn test_stats() {
        let buffer = ClientRingBuffer::with_capacity(1024);

        buffer.log_event(ClientEventType::Info, "test");

        let stats = buffer.stats();
        assert_eq!(stats.capacity_bytes, 1024);
        assert!(stats.bytes_used > 0);
        assert_eq!(stats.entry_count, 1);
        assert_eq!(stats.total_logged, 1);
        assert_eq!(stats.dropped, 0);
    }

    #[test]
    fn test_concurrent_access() {
        use std::{sync::Arc, thread};

        let buffer = Arc::new(ClientRingBuffer::with_capacity(4096));
        let mut handles = vec![];

        // Spawn multiple writers
        for t in 0..4 {
            let buf = Arc::clone(&buffer);
            handles.push(thread::spawn(move || {
                for i in 0..50 {
                    buf.log_event(ClientEventType::Info, format!("thread {t} event {i}"));
                }
            }));
        }

        // Spawn readers
        for _ in 0..2 {
            let buf = Arc::clone(&buffer);
            handles.push(thread::spawn(move || {
                for _ in 0..25 {
                    let _ = buf.tail(10);
                    let _ = buf.stats();
                }
            }));
        }

        for handle in handles {
            handle.join().expect("thread panicked");
        }

        let stats = buffer.stats();
        assert!(stats.total_logged > 0, "Should have logged entries");
    }

    #[test]
    fn test_default() {
        let buffer = ClientRingBuffer::default();
        let stats = buffer.stats();
        assert_eq!(stats.capacity_bytes, DEFAULT_CLIENT_CAPACITY);
    }

    #[test]
    fn test_debug_impl() {
        let buffer = ClientRingBuffer::with_capacity(512);
        buffer.log_event(ClientEventType::Info, "test");

        let debug = format!("{buffer:?}");
        assert!(debug.contains("ClientRingBuffer"));
        assert!(debug.contains("capacity_bytes"));
        assert!(debug.contains("512"));
    }

    #[test]
    fn test_sequence_numbers() {
        let buffer = ClientRingBuffer::new();

        buffer.log_event(ClientEventType::Info, "first");
        buffer.log_event(ClientEventType::Info, "second");
        buffer.log_event(ClientEventType::Info, "third");

        let entries = buffer.entries();
        assert_eq!(entries[0].seq, 0);
        assert_eq!(entries[1].seq, 1);
        assert_eq!(entries[2].seq, 2);
    }

    #[test]
    fn test_timestamps_increase() {
        let buffer = ClientRingBuffer::new();

        buffer.log_event(ClientEventType::Info, "first");
        std::thread::sleep(std::time::Duration::from_millis(10));
        buffer.log_event(ClientEventType::Info, "second");

        let entries = buffer.entries();
        assert!(entries[1].timestamp_us > entries[0].timestamp_us, "Timestamps should increase");
    }

    #[test]
    fn test_bytes_used() {
        let buffer = ClientRingBuffer::new();
        assert_eq!(buffer.bytes_used(), 0);

        buffer.log_event(ClientEventType::Info, "test message");
        assert!(buffer.bytes_used() > 0);
    }

    #[test]
    fn test_clear() {
        let buffer = ClientRingBuffer::new();
        buffer.log_event(ClientEventType::Info, "test");
        assert!(buffer.bytes_used() > 0);

        buffer.clear();
        assert_eq!(buffer.bytes_used(), 0);
        assert_eq!(buffer.stats().entry_count, 0);
    }

    #[test]
    fn test_event_type_display() {
        assert_eq!(ClientEventType::KeyPress.as_str(), "KEY");
        assert_eq!(ClientEventType::CommandExecuted.as_str(), "CMD");
        assert_eq!(ClientEventType::ModeChanged.as_str(), "MODE");
        assert_eq!(ClientEventType::Error.as_str(), "ERROR");
    }

    #[test]
    fn test_helper_methods() {
        let buffer = ClientRingBuffer::new();

        buffer.log_key("j");
        buffer.log_command("delete");
        buffer.log_mode_change("normal", "insert");
        buffer.log_state_change("cursor moved");
        buffer.log_error("something failed");

        let entries = buffer.entries();
        assert_eq!(entries.len(), 5);
        assert_eq!(entries[0].event_type, ClientEventType::KeyPress);
        assert_eq!(entries[1].event_type, ClientEventType::CommandExecuted);
        assert_eq!(entries[2].event_type, ClientEventType::ModeChanged);
        assert_eq!(entries[2].details, "normal -> insert");
        assert_eq!(entries[3].event_type, ClientEventType::StateChanged);
        assert_eq!(entries[4].event_type, ClientEventType::Error);
    }

    #[test]
    fn test_clone() {
        let buffer = ClientRingBuffer::new();
        buffer.log_event(ClientEventType::Info, "test1");
        buffer.log_event(ClientEventType::Info, "test2");

        let cloned = buffer.clone();
        let original_entries = buffer.entries();
        let cloned_entries = cloned.entries();

        assert_eq!(original_entries.len(), cloned_entries.len());
        assert_eq!(original_entries[0].details, cloned_entries[0].details);
    }
}
