use super::*;

/// Mock clipboard for testing.
struct MockClipboard {
    available: bool,
}

impl ClipboardApi for MockClipboard {
    fn copy_to_clipboard(&self, _text: &str) -> bool {
        self.available
    }

    fn paste_from_clipboard(&self) -> Option<String> {
        if self.available {
            Some("clipboard-text".to_string())
        } else {
            None
        }
    }

    fn copy_to_selection(&self, _text: &str) -> bool {
        self.available
    }

    fn paste_from_selection(&self) -> Option<String> {
        if self.available {
            Some("selection-text".to_string())
        } else {
            None
        }
    }
}

#[test]
fn test_clipboard_available() {
    let clip = MockClipboard { available: true };
    assert!(clip.copy_to_clipboard("test"));
    assert_eq!(clip.paste_from_clipboard(), Some("clipboard-text".to_string()));
    assert!(clip.copy_to_selection("test"));
    assert_eq!(clip.paste_from_selection(), Some("selection-text".to_string()));
}

#[test]
fn test_clipboard_unavailable() {
    let clip = MockClipboard { available: false };
    assert!(!clip.copy_to_clipboard("test"));
    assert!(clip.paste_from_clipboard().is_none());
    assert!(!clip.copy_to_selection("test"));
    assert!(clip.paste_from_selection().is_none());
}

#[test]
fn test_trait_is_send() {
    fn assert_send<T: Send>() {}
    assert_send::<MockClipboard>();
}
