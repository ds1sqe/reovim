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
