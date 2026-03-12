use super::*;

#[test]
fn test_sync_mode_default() {
    let mode = SyncMode::default();
    assert!(mode.is_independent());
    assert!(!mode.is_broadcasting());
    assert!(!mode.is_receiving());
}

#[test]
fn test_sync_mode_broadcast() {
    let mode = SyncMode::Broadcast;
    assert!(mode.is_broadcasting());
    assert!(!mode.is_receiving());
    assert!(!mode.is_independent());
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_sync_mode_follow() {
    let mode = SyncMode::follow("client-1");
    assert!(mode.is_receiving());
    assert!(!mode.is_broadcasting());
    if let SyncMode::Follow { target } = mode {
        assert_eq!(target, "client-1");
    } else {
        panic!("Expected Follow mode");
    }
}

#[test]
fn test_sync_mode_accept() {
    let mode = SyncMode::Accept;
    assert!(mode.is_receiving());
    assert!(!mode.is_broadcasting());
}

#[test]
fn test_client_presence_new() {
    let presence = ClientPresence::new("client-123");
    assert_eq!(presence.client_id, "client-123");
    assert_eq!(presence.name, None);
    assert!(presence.is_active);
    assert!(presence.sync_mode.is_independent());
}

#[test]
fn test_client_presence_builder() {
    let presence = ClientPresence::new("client-1")
        .with_name("Alice")
        .with_sync_mode(SyncMode::Broadcast)
        .with_position(1, 10, 5);

    assert_eq!(presence.display_name(), "Alice");
    assert!(presence.sync_mode.is_broadcasting());
    assert_eq!(presence.active_buffer, Some(1));
    assert_eq!(presence.cursor, Some((10, 5)));
}

#[test]
fn test_display_name_fallback() {
    let presence = ClientPresence::new("client-42");
    assert_eq!(presence.display_name(), "client-42");
}
