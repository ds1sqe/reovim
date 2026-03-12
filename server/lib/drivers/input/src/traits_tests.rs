use crate::{ClipboardError, ClipboardProvider};

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_clipboard_provider_is_object_safe() {
    fn _accepts_ref(_: &dyn ClipboardProvider) {}
    fn _accepts_box(_: Box<dyn ClipboardProvider>) {}
}

struct MockClipboard {
    content: std::sync::Mutex<String>,
}

impl MockClipboard {
    fn new() -> Self {
        Self {
            content: std::sync::Mutex::new(String::new()),
        }
    }
}

impl ClipboardProvider for MockClipboard {
    fn read(&self) -> Result<String, ClipboardError> {
        Ok(self.content.lock().unwrap().clone())
    }

    fn write(&mut self, text: &str) -> Result<(), ClipboardError> {
        *self.content.lock().unwrap() = text.to_string();
        Ok(())
    }

    fn is_available(&self) -> bool {
        true
    }

    fn name(&self) -> &'static str {
        "mock"
    }
}

#[test]
fn test_clipboard_provider_impl() {
    let mut clipboard = MockClipboard::new();
    assert!(clipboard.is_available());
    assert_eq!(clipboard.name(), "mock");

    clipboard.write("hello").unwrap();
    assert_eq!(clipboard.read().unwrap(), "hello");
}

#[test]
fn test_clipboard_provider_as_trait_object() {
    let clipboard: Box<dyn ClipboardProvider> = Box::new(MockClipboard::new());
    assert!(clipboard.is_available());
    assert_eq!(clipboard.name(), "mock");
}

#[test]
fn test_clipboard_provider_write_and_read_multiple() {
    let mut clipboard = MockClipboard::new();
    clipboard.write("first").unwrap();
    assert_eq!(clipboard.read().unwrap(), "first");
    clipboard.write("second").unwrap();
    assert_eq!(clipboard.read().unwrap(), "second");
}

#[test]
fn test_clipboard_provider_empty_read() {
    let clipboard = MockClipboard::new();
    assert_eq!(clipboard.read().unwrap(), "");
}
