use super::*;

#[test]
fn test_layout_sync_mode_default() {
    let mode = LayoutSyncMode::default();
    assert!(mode.is_independent());
    assert!(!mode.is_broadcasting());
    assert!(!mode.is_receiving());
}

#[test]
fn test_layout_sync_mode_broadcast() {
    let mode = LayoutSyncMode::Broadcast;
    assert!(mode.is_broadcasting());
    assert!(!mode.is_receiving());
    assert!(!mode.is_independent());
}

#[test]
fn test_layout_sync_mode_follow() {
    let mode = LayoutSyncMode::follow("client-1");
    assert!(mode.is_receiving());
    assert!(!mode.is_broadcasting());
    assert_eq!(mode.follow_target(), Some("client-1"));
}

#[test]
fn test_layout_sync_mode_accept() {
    let mode = LayoutSyncMode::Accept;
    assert!(mode.is_receiving());
    assert!(!mode.is_broadcasting());
    assert_eq!(mode.follow_target(), None);
}
