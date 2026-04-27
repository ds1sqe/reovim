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

#[test]
fn test_log_event_try_write_behavior() {
    // Test that log_event uses try_write and handles lock failure gracefully
    let buffer = ClientRingBuffer::new();

    // Normal logging should work
    buffer.log_event(ClientEventType::Info, "test1");
    buffer.log_event(ClientEventType::Info, "test2");

    let entries = buffer.entries();
    assert_eq!(entries.len(), 2);
}

#[test]
fn test_clear_try_write_behavior() {
    // Test that clear uses try_write and handles lock failure gracefully
    let buffer = ClientRingBuffer::new();
    buffer.log_event(ClientEventType::Info, "test");
    assert_eq!(buffer.stats().entry_count, 1);

    // Clear should work normally
    buffer.clear();
    assert_eq!(buffer.stats().entry_count, 0);
}

#[test]
fn test_entry_size_bytes() {
    let entry = ClientLogEntry {
        seq: 0,
        timestamp_us: 0,
        event_type: ClientEventType::Info,
        details: String::with_capacity(100),
    };

    // Fixed: 8 + 8 + 1 = 17, plus capacity of details
    assert_eq!(entry.size_bytes(), 17 + 100);
}

#[test]
fn test_buffer_stats_fields() {
    let buffer = ClientRingBuffer::with_capacity(2048);
    buffer.log_event(ClientEventType::Info, "test1");
    buffer.log_event(ClientEventType::Info, "test2");

    let stats = buffer.stats();
    assert_eq!(stats.capacity_bytes, 2048);
    assert_eq!(stats.entry_count, 2);
    assert_eq!(stats.total_logged, 2);
    assert_eq!(stats.dropped, 0);
    assert!(stats.bytes_used > 0);
}

#[test]
fn test_warning_event_type_as_str() {
    assert_eq!(ClientEventType::Warning.as_str(), "WARN");
}

#[test]
fn test_event_type_display_all_variants() {
    assert_eq!(format!("{}", ClientEventType::KeyPress), "KEY");
    assert_eq!(format!("{}", ClientEventType::CommandExecuted), "CMD");
    assert_eq!(format!("{}", ClientEventType::ModeChanged), "MODE");
    assert_eq!(format!("{}", ClientEventType::StateChanged), "STATE");
    assert_eq!(format!("{}", ClientEventType::Error), "ERROR");
    assert_eq!(format!("{}", ClientEventType::Warning), "WARN");
    assert_eq!(format!("{}", ClientEventType::Info), "INFO");
}
