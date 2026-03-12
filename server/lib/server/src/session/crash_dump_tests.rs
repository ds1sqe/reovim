use super::*;

#[test]
fn test_crash_dir() {
    let dir = crash_dir();
    assert!(dir.ends_with("reovim/crash") || dir.ends_with("reovim\\crash"));
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_dump_client_to_file() {
    let ring_buffer = ClientRingBuffer::new();
    ring_buffer.log_key("a");
    ring_buffer.log_key("b");
    ring_buffer.log_command("write");

    let result = dump_client_to_file(ClientId::new(42), &ring_buffer);

    // May fail in CI if directory is not writable
    if let Ok(path) = result {
        assert!(path.exists());

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("Client Debug Dump"));
        assert!(content.contains("Client ID: 42"));
        assert!(content.contains("KEY"));
        assert!(content.contains("CMD"));

        std::fs::remove_file(&path).ok();
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_try_dump_client_to_file() {
    let ring_buffer = ClientRingBuffer::new();
    ring_buffer.log_error("test error");

    let result = try_dump_client_to_file(ClientId::new(99), &ring_buffer);

    if let Some(path) = result {
        assert!(path.exists());

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("Client ID: 99"));
        assert!(content.contains("ERROR"));

        std::fs::remove_file(&path).ok();
    }
}
