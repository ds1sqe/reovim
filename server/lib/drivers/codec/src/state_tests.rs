//! Tests for codec session state.

use reovim_kernel::api::v1::BufferId;

use {super::*, crate::ContentType};

fn buf(id: usize) -> BufferId {
    BufferId::from_raw(id)
}

fn test_metadata() -> CodecMetadata {
    CodecMetadata::new(ContentType::new("text/utf-8"))
}

#[test]
fn new_is_empty() {
    let state = CodecSessionState::new();
    assert!(state.is_empty());
    assert_eq!(state.len(), 0);
}

#[test]
fn default_is_empty() {
    let state = CodecSessionState::default();
    assert!(state.is_empty());
}

#[test]
fn session_extension_create() {
    let state = CodecSessionState::create();
    assert!(state.is_empty());
}

#[test]
fn insert_and_get() {
    let mut state = CodecSessionState::new();
    state.insert(buf(1), test_metadata());
    assert_eq!(state.len(), 1);

    let m = state.get(buf(1)).unwrap();
    assert_eq!(m.content_type().as_str(), "text/utf-8");
}

#[test]
fn get_missing() {
    let state = CodecSessionState::new();
    assert!(state.get(buf(1)).is_none());
}

#[test]
fn contains() {
    let mut state = CodecSessionState::new();
    assert!(!state.contains(buf(1)));
    state.insert(buf(1), test_metadata());
    assert!(state.contains(buf(1)));
}

#[test]
fn remove() {
    let mut state = CodecSessionState::new();
    state.insert(buf(1), test_metadata());
    let removed = state.remove(buf(1));
    assert!(removed.is_some());
    assert!(state.is_empty());
}

#[test]
fn remove_missing() {
    let mut state = CodecSessionState::new();
    assert!(state.remove(buf(1)).is_none());
}

#[test]
fn insert_replaces() {
    let mut state = CodecSessionState::new();
    state.insert(buf(1), test_metadata());

    let mut new_meta = CodecMetadata::new(ContentType::new("binary/raw"));
    new_meta.set("key", "value");
    state.insert(buf(1), new_meta);

    assert_eq!(state.len(), 1);
    let m = state.get(buf(1)).unwrap();
    assert_eq!(m.content_type().as_str(), "binary/raw");
}

#[test]
fn multiple_buffers() {
    let mut state = CodecSessionState::new();
    state.insert(buf(1), test_metadata());
    state.insert(buf(2), CodecMetadata::new(ContentType::new("binary/raw")));
    assert_eq!(state.len(), 2);

    assert_eq!(state.get(buf(1)).unwrap().content_type().as_str(), "text/utf-8");
    assert_eq!(state.get(buf(2)).unwrap().content_type().as_str(), "binary/raw");
}

#[test]
fn clear() {
    let mut state = CodecSessionState::new();
    state.insert(buf(1), test_metadata());
    state.insert(buf(2), test_metadata());
    state.clear();
    assert!(state.is_empty());
}

#[test]
fn debug_format() {
    let mut state = CodecSessionState::new();
    state.insert(buf(1), test_metadata());
    let debug = format!("{state:?}");
    assert!(debug.contains("CodecSessionState"));
    assert!(debug.contains("buffer_count"));
    assert!(debug.contains("cached_raw_count"));
    assert!(debug.contains("active_view_count"));
}

// --- Raw bytes cache tests ---

#[test]
fn insert_and_get_raw() {
    let mut state = CodecSessionState::new();
    let data = vec![0x7f, 0x45, 0x4c, 0x46];
    state.insert_raw(buf(1), data.clone());
    assert_eq!(state.get_raw(buf(1)), Some(data.as_slice()));
}

#[test]
fn get_raw_missing() {
    let state = CodecSessionState::new();
    assert!(state.get_raw(buf(1)).is_none());
}

#[test]
fn remove_raw_standalone() {
    let mut state = CodecSessionState::new();
    state.insert_raw(buf(1), vec![1, 2, 3]);
    state.remove_raw(buf(1));
    assert!(state.get_raw(buf(1)).is_none());
}

#[test]
fn remove_clears_raw_and_view() {
    let mut state = CodecSessionState::new();
    state.insert(buf(1), test_metadata());
    state.insert_raw(buf(1), vec![1, 2, 3]);
    state.set_active_view(buf(1), "hex".to_string());

    state.remove(buf(1));

    assert!(state.get(buf(1)).is_none());
    assert!(state.get_raw(buf(1)).is_none());
    assert!(state.active_view(buf(1)).is_none());
}

#[test]
fn clear_clears_raw_and_views() {
    let mut state = CodecSessionState::new();
    state.insert(buf(1), test_metadata());
    state.insert_raw(buf(1), vec![1, 2]);
    state.set_active_view(buf(1), "default".to_string());
    state.insert(buf(2), test_metadata());
    state.insert_raw(buf(2), vec![3, 4]);
    state.set_active_view(buf(2), "hex".to_string());

    state.clear();

    assert!(state.is_empty());
    assert!(state.get_raw(buf(1)).is_none());
    assert!(state.get_raw(buf(2)).is_none());
    assert!(state.active_view(buf(1)).is_none());
    assert!(state.active_view(buf(2)).is_none());
}

// --- Active view tests ---

#[test]
fn active_view_default_none() {
    let state = CodecSessionState::new();
    assert!(state.active_view(buf(1)).is_none());
}

#[test]
fn set_and_get_active_view() {
    let mut state = CodecSessionState::new();
    state.set_active_view(buf(1), "hex".to_string());
    assert_eq!(state.active_view(buf(1)), Some("hex"));
}

#[test]
fn active_view_replaces() {
    let mut state = CodecSessionState::new();
    state.set_active_view(buf(1), "default".to_string());
    state.set_active_view(buf(1), "hex".to_string());
    assert_eq!(state.active_view(buf(1)), Some("hex"));
}

#[test]
fn insert_raw_replaces() {
    let mut state = CodecSessionState::new();
    state.insert_raw(buf(1), vec![1, 2, 3]);
    state.insert_raw(buf(1), vec![4, 5]);
    assert_eq!(state.get_raw(buf(1)), Some([4u8, 5].as_slice()));
}
