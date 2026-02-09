//! Server-level debug ring buffer for kernel events.
//!
// Allow dead code since these are public APIs meant for use by other crates
#![allow(dead_code)]
//!
//! Linux equivalent: `kernel/printk/printk_ringbuffer.c`
//!
//! This module provides a fixed-capacity ring buffer that captures all `pr_*!` macro
//! output for post-mortem analysis. The buffer wraps when full, discarding oldest entries.
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────┐
//! │  DebugRingBuffer (64 KB default)                               │
//! │  ┌───────────────────────────────────────────────────────────┐  │
//! │  │  RwLock<RingBufferInner>                                  │  │
//! │  │  ├── entries: Vec<LogEntry>  [0] [1] [2] ... [N]          │  │
//! │  │  ├── total_logged: u64 (monotonic counter)                │  │
//! │  │  ├── total_logged: u64 (monotonic counter)                │  │
//! │  │  ├── bytes_used: usize (current byte usage)               │  │
//! │  │  └── intern_tables: (targets, files)                      │  │
//! │  └───────────────────────────────────────────────────────────┘  │
//! └─────────────────────────────────────────────────────────────────┘
//! ```
//!
//! # Thread Safety
//!
//! Uses `parking_lot::RwLock` for fast, non-poisoning concurrent access.
//! Multiple readers can access the buffer simultaneously; writers are exclusive.
//!
//! # Panic Safety
//!
//! The `try_dump()` method provides non-blocking access for panic handlers,
//! returning `None` if the lock cannot be acquired immediately.

use std::{collections::HashMap, sync::OnceLock, time::Instant};

use std::fmt::Write;

use parking_lot::RwLock;

use reovim_kernel::api::v1::{Level, Record};

// =============================================================================
// Constants
// =============================================================================

/// Default capacity for debug ring buffer (64 KB).
pub const DEFAULT_CAPACITY: usize = 64 * 1024;

/// Maximum message length before truncation (8 KB).
pub const MAX_MESSAGE_LEN: usize = 8 * 1024;

/// Maximum number of interned strings (u16 limit).
const MAX_INTERN_ENTRIES: usize = u16::MAX as usize;

// =============================================================================
// Error Types
// =============================================================================

/// Error returned when the ring buffer is already initialized.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AlreadyInitialized;

impl std::fmt::Display for AlreadyInitialized {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "debug ring buffer already initialized")
    }
}

impl std::error::Error for AlreadyInitialized {}

// =============================================================================
// Intern Table
// =============================================================================

/// String interning table to reduce memory usage for repeated strings.
///
/// Commonly repeated strings like module paths and file names are stored once
/// and referenced by a 16-bit ID. This significantly reduces memory usage
/// when the same targets/files appear in many log entries.
#[derive(Debug, Default)]
struct InternTable {
    /// ID -> string mapping.
    strings: Vec<String>,
    /// String -> ID mapping for fast lookup.
    map: HashMap<String, u16>,
}

impl InternTable {
    /// Creates a new empty intern table.
    fn new() -> Self {
        Self::default()
    }

    /// Interns a string, returning its ID.
    ///
    /// If the string is already interned, returns the existing ID.
    /// If the table is full (65535 entries), returns 0 (empty string sentinel).
    fn intern(&mut self, s: &str) -> u16 {
        if let Some(&id) = self.map.get(s) {
            return id;
        }

        // Table full - return sentinel value
        if self.strings.len() >= MAX_INTERN_ENTRIES {
            return 0;
        }

        #[allow(clippy::cast_possible_truncation)]
        let id = self.strings.len() as u16;
        self.strings.push(s.to_string());
        self.map.insert(s.to_string(), id);
        id
    }

    /// Gets a string by ID.
    fn get(&self, id: u16) -> &str {
        self.strings.get(id as usize).map_or("", String::as_str)
    }

    /// Returns the number of interned strings.
    #[allow(clippy::missing_const_for_fn)]
    fn len(&self) -> usize {
        self.strings.len()
    }
}

// =============================================================================
// Log Entry
// =============================================================================

/// A single log entry in the ring buffer.
#[derive(Debug, Clone)]
pub struct LogEntry {
    /// Monotonic sequence number for ordering.
    pub seq: u64,
    /// Timestamp in microseconds since process start.
    pub timestamp_us: u64,
    /// Log level.
    pub level: Level,
    /// Interned target string ID.
    target_id: u16,
    /// Interned file path ID.
    file_id: u16,
    /// Line number.
    pub line: u32,
    /// Log message (truncated to `MAX_MESSAGE_LEN`).
    pub message: String,
}

impl LogEntry {
    /// Estimates the memory size of this entry in bytes.
    #[allow(clippy::missing_const_for_fn)]
    fn size_bytes(&self) -> usize {
        // Fixed fields: seq(8) + timestamp_us(8) + level(1) + target_id(2) + file_id(2) + line(4) = 25
        // Plus message heap allocation
        25 + self.message.capacity()
    }
}

/// A view into a log entry with resolved strings.
///
/// This struct owns all its data to avoid lifetime issues with the lock.
#[derive(Debug, Clone)]
pub struct LogEntryView {
    /// Monotonic sequence number.
    pub seq: u64,
    /// Timestamp in microseconds since process start.
    pub timestamp_us: u64,
    /// Log level.
    pub level: Level,
    /// Target module path.
    pub target: String,
    /// Source file path.
    pub file: String,
    /// Line number.
    pub line: u32,
    /// Log message.
    pub message: String,
}

// =============================================================================
// Buffer Statistics
// =============================================================================

/// Statistics about the ring buffer.
#[derive(Debug, Clone, Copy)]
pub struct BufferStats {
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
    /// Number of interned target strings.
    pub interned_targets: usize,
    /// Number of interned file strings.
    pub interned_files: usize,
}

// =============================================================================
// Ring Buffer Inner
// =============================================================================

/// Inner state of the ring buffer, protected by `RwLock`.
#[derive(Debug)]
struct RingBufferInner {
    /// Log entries in circular order.
    entries: Vec<LogEntry>,
    /// Total entries logged (monotonic, never wraps).
    total_logged: u64,
    /// Current byte usage.
    bytes_used: usize,
    /// Maximum capacity in bytes.
    capacity_bytes: usize,
    /// Interned target strings.
    targets: InternTable,
    /// Interned file paths.
    files: InternTable,
    /// Process start time for timestamps.
    start_time: Instant,
}

impl RingBufferInner {
    /// Creates a new ring buffer inner with the given capacity.
    fn new(capacity_bytes: usize) -> Self {
        Self {
            entries: Vec::new(),
            total_logged: 0,
            bytes_used: 0,
            capacity_bytes,
            targets: InternTable::new(),
            files: InternTable::new(),
            start_time: Instant::now(),
        }
    }

    /// Pushes a new entry, evicting old entries if necessary to stay within capacity.
    fn push(&mut self, record: &Record) {
        let target_id = self.targets.intern(record.module_path());
        let file_id = self.files.intern(record.file());

        // Truncate message if too long
        let message = if record.message().len() > MAX_MESSAGE_LEN {
            let mut truncated = record.message()[..MAX_MESSAGE_LEN].to_string();
            truncated.push_str("...[truncated]");
            truncated
        } else {
            record.message().to_string()
        };

        let entry = LogEntry {
            seq: self.total_logged,
            #[allow(clippy::cast_possible_truncation)]
            timestamp_us: self.start_time.elapsed().as_micros() as u64,
            level: record.level(),
            target_id,
            file_id,
            line: record.line(),
            message,
        };

        let entry_size = entry.size_bytes();
        self.total_logged += 1;

        // Evict oldest entries (front of Vec) until we have room
        while self.bytes_used + entry_size > self.capacity_bytes && !self.entries.is_empty() {
            self.bytes_used = self.bytes_used.saturating_sub(self.entries[0].size_bytes());
            self.entries.remove(0);
        }

        // Add new entry
        self.entries.push(entry);
        self.bytes_used += entry_size;
    }

    /// Returns the N most recent entries (newest first).
    fn tail(&self, n: usize) -> Vec<LogEntryView> {
        let count = n.min(self.entries.len());
        let mut result = Vec::with_capacity(count);

        // Entries are stored in insertion order, newest at the end
        for entry in self.entries.iter().rev().take(count) {
            result.push(LogEntryView {
                seq: entry.seq,
                timestamp_us: entry.timestamp_us,
                level: entry.level,
                target: self.targets.get(entry.target_id).to_string(),
                file: self.files.get(entry.file_id).to_string(),
                line: entry.line,
                message: entry.message.clone(),
            });
        }

        result
    }

    /// Returns all entries in order (oldest to newest).
    fn entries(&self) -> Vec<LogEntryView> {
        self.entries
            .iter()
            .map(|entry| LogEntryView {
                seq: entry.seq,
                timestamp_us: entry.timestamp_us,
                level: entry.level,
                target: self.targets.get(entry.target_id).to_string(),
                file: self.files.get(entry.file_id).to_string(),
                line: entry.line,
                message: entry.message.clone(),
            })
            .collect()
    }

    /// Formats all entries as a string for crash dumps.
    fn dump(&self) -> String {
        let mut output = String::new();
        output.push_str("=== Debug Ring Buffer Dump ===\n");
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
            let target = self.targets.get(entry.target_id);
            let file = self.files.get(entry.file_id);
            let _ = writeln!(
                output,
                "[{:>10}us] {:5} {}:{} ({}) {}",
                entry.timestamp_us,
                entry.level.as_str(),
                file,
                entry.line,
                target,
                entry.message
            );
        }

        output.push_str("=== End Dump ===\n");
        output
    }

    /// Returns buffer statistics.
    fn stats(&self) -> BufferStats {
        let dropped = if self.total_logged > self.entries.len() as u64 {
            self.total_logged - self.entries.len() as u64
        } else {
            0
        };

        BufferStats {
            capacity_bytes: self.capacity_bytes,
            bytes_used: self.bytes_used,
            entry_count: self.entries.len(),
            total_logged: self.total_logged,
            dropped,
            interned_targets: self.targets.len(),
            interned_files: self.files.len(),
        }
    }
}

// =============================================================================
// Server Ring Buffer
// =============================================================================

/// Server-level debug ring buffer.
///
/// A fixed-capacity ring buffer that captures kernel log events (`pr_*!` macros)
/// for post-mortem debugging. When the buffer is full, oldest entries are discarded.
///
/// # Thread Safety
///
/// All operations are thread-safe. Multiple readers can access the buffer
/// simultaneously; writers are exclusive.
///
/// # Example
///
/// ```ignore
/// use reovim_server::debug::{init_debug_ring, debug_ring};
///
/// // Initialize at startup
/// init_debug_ring().expect("ring already initialized");
///
/// // Query recent entries
/// let recent = debug_ring().tail(10);
/// for entry in recent {
///     println!("[{}] {}", entry.level, entry.message);
/// }
/// ```
pub struct DebugRingBuffer {
    inner: RwLock<RingBufferInner>,
}

impl DebugRingBuffer {
    /// Creates a new ring buffer with default capacity (64 KB).
    #[must_use]
    pub fn new() -> Self {
        Self::with_capacity(DEFAULT_CAPACITY)
    }

    /// Creates a new ring buffer with the specified capacity in bytes.
    #[must_use]
    pub fn with_capacity(capacity_bytes: usize) -> Self {
        Self {
            inner: RwLock::new(RingBufferInner::new(capacity_bytes)),
        }
    }

    /// Pushes a log record to the buffer.
    ///
    /// If the buffer would exceed capacity, oldest entries are discarded.
    pub fn push(&self, record: &Record) {
        // Use try_write to avoid blocking during panic
        if let Some(mut inner) = self.inner.try_write() {
            inner.push(record);
        }
        // If we can't get the lock, skip this entry (panic safety)
    }

    /// Returns the N most recent entries (newest first).
    #[must_use]
    pub fn tail(&self, n: usize) -> Vec<LogEntryView> {
        self.inner.read().tail(n)
    }

    /// Returns all entries in order (oldest to newest).
    #[must_use]
    pub fn entries(&self) -> Vec<LogEntryView> {
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
    pub fn stats(&self) -> BufferStats {
        self.inner.read().stats()
    }

    /// Returns the current byte usage.
    #[must_use]
    pub fn bytes_used(&self) -> usize {
        self.inner.read().bytes_used
    }
}

impl Default for DebugRingBuffer {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for DebugRingBuffer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let stats = self.stats();
        f.debug_struct("DebugRingBuffer")
            .field("capacity_bytes", &stats.capacity_bytes)
            .field("bytes_used", &stats.bytes_used)
            .field("entry_count", &stats.entry_count)
            .field("total_logged", &stats.total_logged)
            .finish()
    }
}

// =============================================================================
// Global Instance
// =============================================================================

/// Global debug ring buffer instance.
static DEBUG_RING: OnceLock<DebugRingBuffer> = OnceLock::new();

/// Initializes the global debug ring buffer with default capacity.
///
/// # Errors
///
/// Returns `Err(AlreadyInitialized)` if the buffer has already been initialized.
pub fn init_debug_ring() -> Result<(), AlreadyInitialized> {
    init_debug_ring_with_capacity(DEFAULT_CAPACITY)
}

/// Initializes the global debug ring buffer with a custom capacity.
///
/// # Errors
///
/// Returns `Err(AlreadyInitialized)` if the buffer has already been initialized.
pub fn init_debug_ring_with_capacity(capacity_bytes: usize) -> Result<(), AlreadyInitialized> {
    DEBUG_RING
        .set(DebugRingBuffer::with_capacity(capacity_bytes))
        .map_err(|_| AlreadyInitialized)
}

/// Returns a reference to the global debug ring buffer.
///
/// # Panics
///
/// Panics if `init_debug_ring()` has not been called.
#[must_use]
pub fn debug_ring() -> &'static DebugRingBuffer {
    DEBUG_RING
        .get()
        .expect("debug ring buffer not initialized - call init_debug_ring() first")
}

/// Returns a reference to the global debug ring buffer if initialized.
#[must_use]
pub fn try_debug_ring() -> Option<&'static DebugRingBuffer> {
    DEBUG_RING.get()
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn make_record(level: Level, message: &str) -> Record<'_> {
        Record::builder(level)
            .message(message)
            .module_path("test::module")
            .file("test.rs")
            .line(42)
            .build()
    }

    #[test]
    fn test_push_and_tail() {
        let buffer = DebugRingBuffer::with_capacity(4096);

        buffer.push(&make_record(Level::Info, "message 1"));
        buffer.push(&make_record(Level::Warn, "message 2"));
        buffer.push(&make_record(Level::Error, "message 3"));

        let recent = buffer.tail(2);
        assert_eq!(recent.len(), 2);
        assert_eq!(recent[0].message, "message 3"); // Newest first
        assert_eq!(recent[1].message, "message 2");
    }

    #[test]
    fn test_entries_order() {
        let buffer = DebugRingBuffer::with_capacity(4096);

        buffer.push(&make_record(Level::Info, "first"));
        buffer.push(&make_record(Level::Info, "second"));
        buffer.push(&make_record(Level::Info, "third"));

        let all = buffer.entries();
        assert_eq!(all.len(), 3);
        assert_eq!(all[0].message, "first"); // Oldest first
        assert_eq!(all[1].message, "second");
        assert_eq!(all[2].message, "third");
    }

    #[test]
    fn test_wrap_around() {
        // Small buffer that will definitely wrap
        let buffer = DebugRingBuffer::with_capacity(200);

        // Push enough entries to force wrap
        for i in 0..20 {
            buffer.push(&make_record(Level::Info, &format!("message {i}")));
        }

        let stats = buffer.stats();
        assert!(stats.bytes_used <= 200, "Should not exceed capacity");
        assert!(stats.dropped > 0, "Should have dropped entries");
        assert!(stats.total_logged == 20, "Should have logged 20 total");
    }

    #[test]
    fn test_string_interning() {
        let buffer = DebugRingBuffer::with_capacity(4096);

        // Push multiple entries with same target/file
        for i in 0..10 {
            buffer.push(&make_record(Level::Info, &format!("message {i}")));
        }

        let stats = buffer.stats();
        assert_eq!(stats.interned_targets, 1, "Should intern target once");
        assert_eq!(stats.interned_files, 1, "Should intern file once");
    }

    #[test]
    fn test_message_truncation() {
        let buffer = DebugRingBuffer::with_capacity(64 * 1024);

        // Create a message longer than MAX_MESSAGE_LEN
        let long_message = "x".repeat(MAX_MESSAGE_LEN + 1000);
        buffer.push(&make_record(Level::Info, &long_message));

        let entries = buffer.entries();
        assert_eq!(entries.len(), 1);
        assert!(
            entries[0].message.len() <= MAX_MESSAGE_LEN + 20, // +20 for truncation marker
            "Message should be truncated"
        );
        assert!(entries[0].message.ends_with("...[truncated]"), "Should have truncation marker");
    }

    #[test]
    fn test_dump_formatting() {
        let buffer = DebugRingBuffer::with_capacity(4096);

        buffer.push(&make_record(Level::Error, "test error"));
        buffer.push(&make_record(Level::Info, "test info"));

        let dump = buffer.dump();
        assert!(dump.contains("Debug Ring Buffer Dump"));
        assert!(dump.contains("test error"));
        assert!(dump.contains("test info"));
        assert!(dump.contains("ERROR"));
        assert!(dump.contains("INFO"));
    }

    #[test]
    fn test_try_dump() {
        let buffer = DebugRingBuffer::with_capacity(4096);
        buffer.push(&make_record(Level::Info, "test"));

        // Should succeed when no one holds the lock
        let dump = buffer.try_dump();
        assert!(dump.is_some());
        assert!(dump.unwrap().contains("test"));
    }

    #[test]
    fn test_stats() {
        let buffer = DebugRingBuffer::with_capacity(1024);

        buffer.push(&make_record(Level::Info, "test"));

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

        let buffer = Arc::new(DebugRingBuffer::with_capacity(8192));
        let mut handles = vec![];

        // Spawn multiple writers
        for t in 0..4 {
            let buf = Arc::clone(&buffer);
            handles.push(thread::spawn(move || {
                for i in 0..100 {
                    let msg = format!("thread {t} message {i}");
                    let record = Record::builder(Level::Info)
                        .message(&msg)
                        .module_path("test")
                        .file("test.rs")
                        .line(1)
                        .build();
                    buf.push(&record);
                }
            }));
        }

        // Spawn readers
        for _ in 0..2 {
            let buf = Arc::clone(&buffer);
            handles.push(thread::spawn(move || {
                for _ in 0..50 {
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
        let buffer = DebugRingBuffer::default();
        let stats = buffer.stats();
        assert_eq!(stats.capacity_bytes, DEFAULT_CAPACITY);
    }

    #[test]
    fn test_debug_impl() {
        let buffer = DebugRingBuffer::with_capacity(1024);
        buffer.push(&make_record(Level::Info, "test"));

        let debug = format!("{buffer:?}");
        assert!(debug.contains("DebugRingBuffer"));
        assert!(debug.contains("capacity_bytes"));
        assert!(debug.contains("1024"));
    }

    #[test]
    fn test_intern_table() {
        let mut table = InternTable::new();

        // First intern
        let id1 = table.intern("hello");
        assert_eq!(id1, 0);

        // Same string returns same ID
        let id2 = table.intern("hello");
        assert_eq!(id2, 0);

        // Different string gets new ID
        let id3 = table.intern("world");
        assert_eq!(id3, 1);

        // Lookup works
        assert_eq!(table.get(0), "hello");
        assert_eq!(table.get(1), "world");
        assert_eq!(table.get(99), ""); // Invalid ID returns empty
    }

    #[test]
    fn test_sequence_numbers() {
        let buffer = DebugRingBuffer::with_capacity(4096);

        buffer.push(&make_record(Level::Info, "first"));
        buffer.push(&make_record(Level::Info, "second"));
        buffer.push(&make_record(Level::Info, "third"));

        let entries = buffer.entries();
        assert_eq!(entries[0].seq, 0);
        assert_eq!(entries[1].seq, 1);
        assert_eq!(entries[2].seq, 2);
    }

    #[test]
    fn test_timestamps_increase() {
        let buffer = DebugRingBuffer::with_capacity(4096);

        buffer.push(&make_record(Level::Info, "first"));
        std::thread::sleep(std::time::Duration::from_millis(10));
        buffer.push(&make_record(Level::Info, "second"));

        let entries = buffer.entries();
        assert!(entries[1].timestamp_us > entries[0].timestamp_us, "Timestamps should increase");
    }

    #[test]
    fn test_bytes_used() {
        let buffer = DebugRingBuffer::with_capacity(4096);
        assert_eq!(buffer.bytes_used(), 0);

        buffer.push(&make_record(Level::Info, "test message"));
        assert!(buffer.bytes_used() > 0);
    }

    #[test]
    fn test_already_initialized_error() {
        let err = AlreadyInitialized;
        assert_eq!(format!("{err}"), "debug ring buffer already initialized");
    }

    #[test]
    fn test_intern_table_full() {
        let mut table = InternTable::new();

        // Fill the table to MAX_INTERN_ENTRIES
        for i in 0..MAX_INTERN_ENTRIES {
            let id = table.intern(&format!("string_{i}"));
            assert_eq!(id, u16::try_from(i).unwrap());
        }

        // Next intern should return sentinel value 0
        let overflow_id = table.intern("overflow");
        assert_eq!(overflow_id, 0);

        // Original string 0 should still work
        assert_eq!(table.get(0), "string_0");
    }

    #[test]
    fn test_eviction_edge_cases() {
        // Very small buffer to test edge cases in eviction
        let buffer = DebugRingBuffer::with_capacity(100);

        // Push one entry
        buffer.push(&make_record(Level::Info, "first"));
        let stats1 = buffer.stats();
        assert_eq!(stats1.entry_count, 1);

        // Push entries until we trigger eviction
        for i in 0..10 {
            buffer.push(&make_record(Level::Info, &format!("msg{i}")));
        }

        let stats2 = buffer.stats();
        assert!(stats2.bytes_used <= 100);
        assert!(stats2.dropped > 0);
    }

    #[test]
    fn test_eviction_with_large_entry() {
        let buffer = DebugRingBuffer::with_capacity(200);

        // Add several entries
        for i in 0..5 {
            buffer.push(&make_record(Level::Info, &format!("entry_{i}")));
        }

        // Add a large entry that will force multiple evictions
        let large_msg = "x".repeat(150);
        buffer.push(&make_record(Level::Info, &large_msg));

        let stats = buffer.stats();
        assert!(stats.bytes_used <= 200);
        assert!(stats.dropped > 0);
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_global_init_and_access() {
        use std::sync::Mutex;

        // Use a mutex to ensure this test doesn't interfere with others
        static TEST_LOCK: Mutex<()> = Mutex::new(());
        let _guard = TEST_LOCK.lock().unwrap();

        // try_debug_ring should return None if not initialized
        if try_debug_ring().is_none() {
            // Initialize with custom capacity
            let result = init_debug_ring_with_capacity(2048);
            if result.is_ok() {
                // Access the ring
                let ring = debug_ring();
                ring.push(&make_record(Level::Info, "test"));

                let stats = ring.stats();
                assert_eq!(stats.capacity_bytes, 2048);

                // Try to initialize again - should fail
                let result2 = init_debug_ring_with_capacity(4096);
                assert!(result2.is_err());
                assert_eq!(result2.unwrap_err(), AlreadyInitialized);

                // try_debug_ring should now return Some
                assert!(try_debug_ring().is_some());
            }
        }
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_debug_ring_uninitialized_returns_none_via_try() {
        // OnceLock is global state shared across tests, so we cannot reliably
        // test the panic path of debug_ring() (another test may have initialized it).
        // Instead, verify that try_debug_ring() returns a consistent result:
        // either Some (already initialized by another test) or None (first test to run).
        let result = try_debug_ring();
        if result.is_none() {
            // Not yet initialized: verify try_debug_ring consistently returns None
            assert!(try_debug_ring().is_none());
        } else {
            // Already initialized by another test: verify debug_ring() works
            let ring = debug_ring();
            assert!(ring.stats().capacity_bytes > 0);
        }
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_init_debug_ring_default_capacity() {
        use std::sync::Mutex;

        static TEST_LOCK: Mutex<()> = Mutex::new(());
        let _guard = TEST_LOCK.lock().unwrap();

        if try_debug_ring().is_none() {
            let result = init_debug_ring();
            if result.is_ok() {
                let ring = debug_ring();
                let stats = ring.stats();
                assert_eq!(stats.capacity_bytes, DEFAULT_CAPACITY);
            }
        }
    }

    #[test]
    fn test_already_initialized_implements_error() {
        let err = AlreadyInitialized;
        let err_trait: &dyn std::error::Error = &err;
        let _ = err_trait;
        assert_eq!(err, AlreadyInitialized);
    }

    #[test]
    fn test_intern_table_len() {
        let mut table = InternTable::new();
        assert_eq!(table.len(), 0);

        table.intern("first");
        assert_eq!(table.len(), 1);

        table.intern("first"); // Same string
        assert_eq!(table.len(), 1);

        table.intern("second");
        assert_eq!(table.len(), 2);
    }

    #[test]
    fn test_eviction_empty_buffer() {
        let buffer = DebugRingBuffer::with_capacity(50);

        // Push one large entry that exceeds capacity
        let large = "x".repeat(100);
        buffer.push(&make_record(Level::Info, &large));

        // Should handle gracefully - might end up empty if entry too large
        let stats = buffer.stats();
        assert!(stats.bytes_used <= 50 || stats.entry_count == 1);
    }
}
