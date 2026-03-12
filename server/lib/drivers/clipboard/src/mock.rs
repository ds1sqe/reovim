//! Mock clipboard provider for headless testing.
//!
//! In-memory implementation of `ClipboardProvider` that works without
//! a display server. Used by tests across the workspace that need
//! clipboard functionality in CI.

use {
    crate::{ClipboardError, ClipboardProvider},
    std::sync::RwLock,
};

/// In-memory clipboard provider for headless testing.
///
/// Stores clipboard and selection content in `RwLock<String>` — no
/// display server or clipboard tool required.
///
/// # Example
///
/// ```
/// use reovim_driver_clipboard::{MockClipboardProvider, ClipboardProvider};
///
/// let clip = MockClipboardProvider::new();
/// clip.copy_to_clipboard("hello").unwrap();
/// assert_eq!(clip.paste_from_clipboard().unwrap(), Some("hello".to_owned()));
/// ```
pub struct MockClipboardProvider {
    clipboard: RwLock<String>,
    selection: RwLock<String>,
}

impl MockClipboardProvider {
    /// Create a new empty mock clipboard.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            clipboard: RwLock::new(String::new()),
            selection: RwLock::new(String::new()),
        }
    }
}

impl Default for MockClipboardProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl ClipboardProvider for MockClipboardProvider {
    fn clipboard_available(&self) -> bool {
        true
    }

    fn copy_to_clipboard(&self, text: &str) -> Result<(), ClipboardError> {
        text.clone_into(&mut self.clipboard.write().expect("lock poisoned"));
        Ok(())
    }

    fn paste_from_clipboard(&self) -> Result<Option<String>, ClipboardError> {
        let text = self.clipboard.read().expect("lock poisoned").clone();
        if text.is_empty() {
            Ok(None)
        } else {
            Ok(Some(text))
        }
    }

    fn selection_available(&self) -> bool {
        true
    }

    fn copy_to_selection(&self, text: &str) -> Result<(), ClipboardError> {
        text.clone_into(&mut self.selection.write().expect("lock poisoned"));
        Ok(())
    }

    fn paste_from_selection(&self) -> Result<Option<String>, ClipboardError> {
        let text = self.selection.read().expect("lock poisoned").clone();
        if text.is_empty() {
            Ok(None)
        } else {
            Ok(Some(text))
        }
    }
}

impl std::fmt::Debug for MockClipboardProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MockClipboardProvider")
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
#[path = "mock_tests.rs"]
mod tests;
