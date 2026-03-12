use super::*;

#[test]
fn test_overlay_sync_mode_default() {
    let mode = OverlaySyncMode::default();
    assert!(mode.is_local());
    assert!(!mode.is_shared());
}

#[test]
fn test_overlay_sync_mode_local() {
    let mode = OverlaySyncMode::Local;
    assert!(mode.is_local());
    assert!(!mode.is_shared());
}

#[test]
fn test_overlay_sync_mode_shared() {
    let mode = OverlaySyncMode::Shared;
    assert!(mode.is_shared());
    assert!(!mode.is_local());
}
