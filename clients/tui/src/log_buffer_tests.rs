use super::*;

#[test]
fn test_buffer_capacity_enforcement() {
    let mut buffer = TuiLogBuffer::new(3);

    for i in 0..5 {
        buffer.push(TuiLogEntry::client_log(LogLevel::Info, &format!("msg{i}")));
    }

    assert_eq!(buffer.len(), 3);
    assert_eq!(buffer.total_written(), 5);
}

#[test]
fn test_ring_buffer_overflow() {
    let mut buffer = TuiLogBuffer::new(3);

    for i in 0..5 {
        buffer.push(TuiLogEntry::client_log(LogLevel::Info, &format!("msg{i}")));
    }

    // Should have the last 3 messages
    let tail = buffer.tail(10);
    assert_eq!(tail.len(), 3);
    assert_eq!(tail[0].message, "msg4"); // newest
    assert_eq!(tail[1].message, "msg3");
    assert_eq!(tail[2].message, "msg2"); // oldest
}

#[test]
fn test_tail_returns_newest() {
    let mut buffer = TuiLogBuffer::new(10);

    for i in 0..5 {
        buffer.push(TuiLogEntry::client_log(LogLevel::Info, &format!("msg{i}")));
    }

    let tail = buffer.tail(3);
    assert_eq!(tail.len(), 3);
    assert_eq!(tail[0].message, "msg4"); // newest first
    assert_eq!(tail[1].message, "msg3");
    assert_eq!(tail[2].message, "msg2");
}

#[test]
fn test_clear_empties_buffer() {
    let mut buffer = TuiLogBuffer::new(10);

    buffer.push(TuiLogEntry::client_log(LogLevel::Info, "test"));
    buffer.push(TuiLogEntry::client_log(LogLevel::Warn, "test2"));

    assert_eq!(buffer.len(), 2);

    buffer.clear();

    assert!(buffer.is_empty());
    assert_eq!(buffer.len(), 0);
}

#[test]
fn test_log_source_prefix() {
    let server_entry = TuiLogEntry::new(
        "12:00:00".to_string(),
        LogLevel::Info,
        "test".to_string(),
        "msg".to_string(),
        LogSource::Server,
    );
    assert_eq!(server_entry.source_prefix(), "[S]");

    let client_entry = TuiLogEntry::client_log(LogLevel::Info, "msg");
    assert_eq!(client_entry.source_prefix(), "[C]");
}

#[test]
fn test_level_color() {
    let error = TuiLogEntry::client_log(LogLevel::Error, "err");
    assert_eq!(error.level_color(), LevelColor::Red);

    let warn = TuiLogEntry::client_log(LogLevel::Warn, "warn");
    assert_eq!(warn.level_color(), LevelColor::Yellow);

    let info = TuiLogEntry::client_log(LogLevel::Info, "info");
    assert_eq!(info.level_color(), LevelColor::Default);

    let debug = TuiLogEntry::client_log(LogLevel::Debug, "debug");
    assert_eq!(debug.level_color(), LevelColor::Dim);

    let trace = TuiLogEntry::client_log(LogLevel::Trace, "trace");
    assert_eq!(trace.level_color(), LevelColor::Dim);
}

#[test]
fn test_entries_order_not_wrapped() {
    let mut buffer = TuiLogBuffer::new(5);
    for i in 0..3 {
        buffer.push(TuiLogEntry::client_log(LogLevel::Info, &format!("msg{i}")));
    }

    let entries = buffer.entries();
    assert_eq!(entries.len(), 3);
    assert_eq!(entries[0].message, "msg0");
    assert_eq!(entries[1].message, "msg1");
    assert_eq!(entries[2].message, "msg2");
}

#[test]
fn test_entries_order_after_wrap() {
    let mut buffer = TuiLogBuffer::new(3);
    for i in 0..5 {
        buffer.push(TuiLogEntry::client_log(LogLevel::Info, &format!("msg{i}")));
    }

    // Buffer wrapped: should return oldest to newest (msg2, msg3, msg4)
    let entries = buffer.entries();
    assert_eq!(entries.len(), 3);
    assert_eq!(entries[0].message, "msg2");
    assert_eq!(entries[1].message, "msg3");
    assert_eq!(entries[2].message, "msg4");
}

#[test]
fn test_tail_empty_buffer() {
    let buffer = TuiLogBuffer::new(5);
    let tail = buffer.tail(3);
    assert!(tail.is_empty());
}

#[test]
fn test_tail_zero_count() {
    let mut buffer = TuiLogBuffer::new(5);
    buffer.push(TuiLogEntry::client_log(LogLevel::Info, "msg"));
    let tail = buffer.tail(0);
    assert!(tail.is_empty());
}

#[test]
fn test_tail_after_wrap() {
    let mut buffer = TuiLogBuffer::new(3);
    // Push 5 entries into capacity-3 buffer
    for i in 0..5 {
        buffer.push(TuiLogEntry::client_log(LogLevel::Info, &format!("msg{i}")));
    }

    // write_pos is at 2 (5 % 3 = 2), buffer has [msg3, msg4, msg2]
    // tail(2) should return msg4 (newest), msg3
    let tail = buffer.tail(2);
    assert_eq!(tail.len(), 2);
    assert_eq!(tail[0].message, "msg4"); // newest
    assert_eq!(tail[1].message, "msg3");
}

#[test]
fn test_tail_wraps_around_write_pos() {
    let mut buffer = TuiLogBuffer::new(4);
    // Push exactly 4 entries to fill buffer, then one more to wrap
    for i in 0..5 {
        buffer.push(TuiLogEntry::client_log(LogLevel::Info, &format!("msg{i}")));
    }

    // write_pos = 1 (5 % 4), buffer = [msg4, msg1, msg2, msg3]
    // tail(3) should get: msg4 (idx 0, newest), msg3 (idx 3), msg2 (idx 2)
    let tail = buffer.tail(3);
    assert_eq!(tail.len(), 3);
    assert_eq!(tail[0].message, "msg4");
    assert_eq!(tail[1].message, "msg3");
    assert_eq!(tail[2].message, "msg2");
}

#[test]
fn test_new_entry_constructor() {
    let entry = TuiLogEntry::new(
        "14:30:00".to_string(),
        LogLevel::Warn,
        "kernel::mm".to_string(),
        "allocation failed".to_string(),
        LogSource::Server,
    );

    assert_eq!(entry.timestamp, "14:30:00");
    assert_eq!(entry.level, LogLevel::Warn);
    assert_eq!(entry.target, "kernel::mm");
    assert_eq!(entry.message, "allocation failed");
    assert_eq!(entry.source_prefix(), "[S]");
}

#[test]
fn test_client_log_timestamp_format() {
    let entry = TuiLogEntry::client_log(LogLevel::Info, "test message");
    // Timestamp should be in HH:MM:SS format
    assert_eq!(entry.timestamp.len(), 8);
    assert_eq!(entry.timestamp.chars().filter(|&c| c == ':').count(), 2);
    assert_eq!(entry.target, "tui");
    assert_eq!(entry.message, "test message");
    assert_eq!(entry.source, LogSource::Client);
}

#[test]
fn test_level_color_ansi_codes() {
    assert_eq!(LevelColor::Red.ansi_code(), "\x1b[31m");
    assert_eq!(LevelColor::Yellow.ansi_code(), "\x1b[33m");
    assert_eq!(LevelColor::Default.ansi_code(), "\x1b[0m");
    assert_eq!(LevelColor::Dim.ansi_code(), "\x1b[90m");
}

#[test]
fn test_level_color_reset_code() {
    assert_eq!(LevelColor::reset_code(), "\x1b[0m");
}

#[test]
fn test_buffer_default() {
    let buffer = TuiLogBuffer::default();
    assert!(buffer.is_empty());
    assert_eq!(buffer.capacity(), DEFAULT_TUI_LOG_CAPACITY);
    assert_eq!(buffer.total_written(), 0);
}

#[test]
fn test_clear_preserves_total_written() {
    let mut buffer = TuiLogBuffer::new(10);
    buffer.push(TuiLogEntry::client_log(LogLevel::Info, "a"));
    buffer.push(TuiLogEntry::client_log(LogLevel::Info, "b"));
    assert_eq!(buffer.total_written(), 2);

    buffer.clear();
    assert!(buffer.is_empty());
    assert_eq!(buffer.total_written(), 2); // Preserved for statistics
}

#[test]
fn test_entry_clone() {
    let entry = TuiLogEntry::client_log(LogLevel::Error, "oops");
    let cloned = entry.clone();
    assert_eq!(entry.message, cloned.message);
    assert_eq!(entry.level, cloned.level);
}

#[test]
fn test_entry_debug() {
    let entry = TuiLogEntry::client_log(LogLevel::Info, "test");
    let debug = format!("{entry:?}");
    assert!(debug.contains("TuiLogEntry"));
}

#[test]
fn test_level_color_debug_clone_eq() {
    let color = LevelColor::Red;
    let cloned = color;
    assert_eq!(color, cloned);
    let debug = format!("{color:?}");
    assert!(debug.contains("Red"));
}

#[test]
fn test_buffer_debug() {
    let buffer = TuiLogBuffer::new(5);
    let debug = format!("{buffer:?}");
    assert!(debug.contains("TuiLogBuffer"));
}

#[test]
fn test_tail_request_more_than_buffer() {
    let mut buffer = TuiLogBuffer::new(3);
    buffer.push(TuiLogEntry::client_log(LogLevel::Info, "msg0"));
    buffer.push(TuiLogEntry::client_log(LogLevel::Info, "msg1"));

    // Request more entries than exist
    let tail = buffer.tail(10);
    assert_eq!(tail.len(), 2);
    assert_eq!(tail[0].message, "msg1");
    assert_eq!(tail[1].message, "msg0");
}

#[test]
fn test_entries_exactly_at_capacity() {
    let mut buffer = TuiLogBuffer::new(3);
    buffer.push(TuiLogEntry::client_log(LogLevel::Info, "msg0"));
    buffer.push(TuiLogEntry::client_log(LogLevel::Info, "msg1"));
    buffer.push(TuiLogEntry::client_log(LogLevel::Info, "msg2"));

    // Exactly at capacity, not wrapped yet
    let entries = buffer.entries();
    assert_eq!(entries.len(), 3);
    assert_eq!(entries[0].message, "msg0");
    assert_eq!(entries[2].message, "msg2");
}
