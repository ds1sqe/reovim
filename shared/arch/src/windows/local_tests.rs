use super::*;

#[test]
fn test_process_exists_current() {
    // Current process should exist
    let pid = std::process::id();
    assert!(process_exists(pid));
}

#[test]
fn test_process_exists_nonexistent() {
    // A very high PID likely doesn't exist
    assert!(!process_exists(u32::MAX - 1));
}
