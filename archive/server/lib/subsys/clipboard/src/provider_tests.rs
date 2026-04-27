use super::*;

/// Mock clipboard for testing default methods.
struct MockClipboard {
    available: bool,
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl ClipboardProvider for MockClipboard {
    fn clipboard_available(&self) -> bool {
        self.available
    }
    fn copy_to_clipboard(&self, _text: &str) -> Result<(), ClipboardError> {
        Ok(())
    }
    fn paste_from_clipboard(&self) -> Result<Option<String>, ClipboardError> {
        Ok(Some("clipboard".into()))
    }
    fn selection_available(&self) -> bool {
        false
    }
    fn copy_to_selection(&self, _text: &str) -> Result<(), ClipboardError> {
        Err(ClipboardError::NotAvailable("no selection".into()))
    }
    fn paste_from_selection(&self) -> Result<Option<String>, ClipboardError> {
        Err(ClipboardError::NotAvailable("no selection".into()))
    }
}

#[test]
fn any_clipboard_available_with_clipboard() {
    let clip = MockClipboard { available: true };
    assert!(clip.any_clipboard_available());
}

#[test]
fn any_clipboard_available_without_either() {
    let clip = MockClipboard { available: false };
    assert!(!clip.any_clipboard_available());
}

#[test]
fn clipboard_available_true() {
    let clip = MockClipboard { available: true };
    assert!(clip.clipboard_available());
}

#[test]
fn clipboard_available_false() {
    let clip = MockClipboard { available: false };
    assert!(!clip.clipboard_available());
}

#[test]
fn selection_not_available() {
    let clip = MockClipboard { available: true };
    assert!(!clip.selection_available());
}

#[test]
fn copy_to_clipboard_succeeds() {
    let clip = MockClipboard { available: true };
    assert!(clip.copy_to_clipboard("text").is_ok());
}

#[test]
fn paste_from_clipboard_returns_content() {
    let clip = MockClipboard { available: true };
    let result = clip.paste_from_clipboard().unwrap();
    assert_eq!(result, Some("clipboard".to_string()));
}

#[test]
fn copy_to_selection_fails() {
    let clip = MockClipboard { available: true };
    assert!(clip.copy_to_selection("text").is_err());
}

#[test]
fn paste_from_selection_fails() {
    let clip = MockClipboard { available: true };
    assert!(clip.paste_from_selection().is_err());
}

#[test]
fn provider_is_send_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<MockClipboard>();
}

// ========================================================================
// any_clipboard_available with selection available
// ========================================================================

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn any_clipboard_available_with_selection_only() {
    struct SelectionOnlyClipboard;
    impl ClipboardProvider for SelectionOnlyClipboard {
        fn clipboard_available(&self) -> bool {
            false
        }
        fn copy_to_clipboard(&self, _text: &str) -> Result<(), ClipboardError> {
            Err(ClipboardError::NotAvailable("none".into()))
        }
        fn paste_from_clipboard(&self) -> Result<Option<String>, ClipboardError> {
            Err(ClipboardError::NotAvailable("none".into()))
        }
        fn selection_available(&self) -> bool {
            true
        }
        fn copy_to_selection(&self, _text: &str) -> Result<(), ClipboardError> {
            Ok(())
        }
        fn paste_from_selection(&self) -> Result<Option<String>, ClipboardError> {
            Ok(Some("selection text".into()))
        }
    }

    let clip = SelectionOnlyClipboard;
    assert!(!clip.clipboard_available());
    assert!(clip.selection_available());
    assert!(clip.any_clipboard_available());
    assert!(clip.copy_to_selection("test").is_ok());
    let sel = clip.paste_from_selection().unwrap();
    assert_eq!(sel, Some("selection text".to_string()));
}

// ========================================================================
// Clipboard errors from methods
// ========================================================================

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn copy_to_selection_error_message() {
    let clip = MockClipboard { available: true };
    let err = clip.copy_to_selection("text").unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("not available") || msg.contains("no selection"));
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn paste_from_selection_error_message() {
    let clip = MockClipboard { available: true };
    let err = clip.paste_from_selection().unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("not available") || msg.contains("no selection"));
}

// ========================================================================
// Trait object safety
// ========================================================================

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn clipboard_provider_is_object_safe() {
    fn _accepts_ref(_: &dyn ClipboardProvider) {}
    fn _accepts_box(_: Box<dyn ClipboardProvider>) {}
    fn _accepts_arc(_: std::sync::Arc<dyn ClipboardProvider>) {}
}

// ========================================================================
// Paste from clipboard returns None
// ========================================================================

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn paste_from_clipboard_empty() {
    struct EmptyClipboard;
    impl ClipboardProvider for EmptyClipboard {
        fn clipboard_available(&self) -> bool {
            true
        }
        fn copy_to_clipboard(&self, _text: &str) -> Result<(), ClipboardError> {
            Ok(())
        }
        fn paste_from_clipboard(&self) -> Result<Option<String>, ClipboardError> {
            Ok(None)
        }
        fn selection_available(&self) -> bool {
            false
        }
        fn copy_to_selection(&self, _text: &str) -> Result<(), ClipboardError> {
            Err(ClipboardError::NotAvailable("none".into()))
        }
        fn paste_from_selection(&self) -> Result<Option<String>, ClipboardError> {
            Err(ClipboardError::NotAvailable("none".into()))
        }
    }

    let clip = EmptyClipboard;
    assert!(clip.clipboard_available());
    let result = clip.paste_from_clipboard().unwrap();
    assert!(result.is_none());
    assert!(!clip.any_clipboard_available() || clip.clipboard_available());
}
