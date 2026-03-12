use super::*;

#[test]
fn test_input_source_creation() {
    let _source = UnixInputSource::new();
}

#[test]
fn test_poll_no_input() {
    let mut source = UnixInputSource::new();
    // Should return false immediately with zero timeout (no input available)
    let result = source.poll(Duration::ZERO);
    // In a TTY environment this should work; in CI it may fail
    // but should not panic
    let _ = result;
}

#[test]
fn test_input_source_default() {
    fn assert_default<T: Default>() {}
    assert_default::<UnixInputSource>();
}

#[test]
fn test_input_source_drain_empty() {
    let mut source = UnixInputSource::new();
    // drain() is the default method on InputSource trait
    // In CI, there should be no pending events, so drain returns empty
    let events = source.drain();
    // We can only assert it doesn't panic; events may or may not be empty
    // depending on the test environment
    let _ = events;
}
