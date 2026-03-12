use super::*;

#[test]
fn test_overlay_content_key() {
    let key = OverlayContentKey::new("which-key");
    assert_eq!(key.name(), "which-key");
}

#[test]
fn test_overlay_content_key_equality() {
    let key1 = OverlayContentKey::new("which-key");
    let key2 = OverlayContentKey::new("which-key");
    let key3 = OverlayContentKey::new("diagnostics");

    assert_eq!(key1, key2);
    assert_ne!(key1, key3);
}

#[test]
fn test_overlay_content_key_display() {
    let key = OverlayContentKey::new("which-key");
    assert_eq!(format!("{key}"), "OverlayContentKey(which-key)");
}

#[test]
fn test_overlay_content_key_service_name() {
    assert_eq!(OverlayContentKey::service_name(), "OverlayContent");
}

// Storage tests

#[test]
fn test_storage_set_and_get() {
    let storage = OverlayContentStorage::new();
    let window_id = WindowId::from_raw(1);

    storage.set_content(window_id, vec!["line 1".into(), "line 2".into()]);

    let content = storage.content_for(window_id);
    assert!(content.is_some());
    let lines = content.unwrap();
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0], "line 1");
    assert_eq!(lines[1], "line 2");
}

#[test]
fn test_storage_unknown_window() {
    let storage = OverlayContentStorage::new();
    let window_id = WindowId::from_raw(99);

    assert!(storage.content_for(window_id).is_none());
}

#[test]
fn test_storage_remove() {
    let storage = OverlayContentStorage::new();
    let window_id = WindowId::from_raw(1);

    storage.set_content(window_id, vec!["content".into()]);
    assert!(storage.content_for(window_id).is_some());

    storage.remove(window_id);
    assert!(storage.content_for(window_id).is_none());
}

#[test]
fn test_storage_replace() {
    let storage = OverlayContentStorage::new();
    let window_id = WindowId::from_raw(1);

    storage.set_content(window_id, vec!["old".into()]);
    storage.set_content(window_id, vec!["new".into()]);

    let content = storage.content_for(window_id).unwrap();
    assert_eq!(content.len(), 1);
    assert_eq!(content[0], "new");
}

#[test]
fn test_storage_has_content() {
    let storage = OverlayContentStorage::new();
    let window_id = WindowId::from_raw(1);

    assert!(!storage.has_content(window_id));

    storage.set_content(window_id, vec!["content".into()]);
    assert!(storage.has_content(window_id));

    storage.remove(window_id);
    assert!(!storage.has_content(window_id));
}

#[test]
fn test_storage_multiple_windows() {
    let storage = OverlayContentStorage::new();
    let window1 = WindowId::from_raw(1);
    let window2 = WindowId::from_raw(2);

    storage.set_content(window1, vec!["window 1".into()]);
    storage.set_content(window2, vec!["window 2".into()]);

    assert_eq!(storage.content_for(window1).unwrap()[0], "window 1");
    assert_eq!(storage.content_for(window2).unwrap()[0], "window 2");
}
