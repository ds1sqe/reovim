use super::*;

#[test]
fn new_clipboard_is_empty() {
    let clip = MockClipboardProvider::new();
    assert!(clip.paste_from_clipboard().unwrap().is_none());
    assert!(clip.paste_from_selection().unwrap().is_none());
}

#[test]
fn clipboard_roundtrip() {
    let clip = MockClipboardProvider::new();
    clip.copy_to_clipboard("hello").unwrap();
    assert_eq!(
        clip.paste_from_clipboard().unwrap(),
        Some("hello".to_owned())
    );
}

#[test]
fn selection_roundtrip() {
    let clip = MockClipboardProvider::new();
    clip.copy_to_selection("world").unwrap();
    assert_eq!(
        clip.paste_from_selection().unwrap(),
        Some("world".to_owned())
    );
}

#[test]
fn clipboard_and_selection_independent() {
    let clip = MockClipboardProvider::new();
    clip.copy_to_clipboard("clip").unwrap();
    clip.copy_to_selection("sel").unwrap();
    assert_eq!(
        clip.paste_from_clipboard().unwrap(),
        Some("clip".to_owned())
    );
    assert_eq!(
        clip.paste_from_selection().unwrap(),
        Some("sel".to_owned())
    );
}

#[test]
fn overwrite_clipboard() {
    let clip = MockClipboardProvider::new();
    clip.copy_to_clipboard("first").unwrap();
    clip.copy_to_clipboard("second").unwrap();
    assert_eq!(
        clip.paste_from_clipboard().unwrap(),
        Some("second".to_owned())
    );
}

#[test]
fn always_available() {
    let clip = MockClipboardProvider::new();
    assert!(clip.clipboard_available());
    assert!(clip.selection_available());
    assert!(clip.any_clipboard_available());
}

#[test]
fn default_same_as_new() {
    let clip = MockClipboardProvider::default();
    assert!(clip.clipboard_available());
    assert!(clip.paste_from_clipboard().unwrap().is_none());
}

#[test]
fn debug_impl() {
    let clip = MockClipboardProvider::new();
    let debug = format!("{clip:?}");
    assert!(debug.contains("MockClipboardProvider"));
}
