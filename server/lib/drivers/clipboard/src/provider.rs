//! Clipboard provider trait.

use {crate::ClipboardError, reovim_kernel::api::v1::RegisterContent};

/// Clipboard provider interface for system clipboard and yank history.
///
/// This trait extends the kernel's `RegisterBank` (which handles `""` and `a-z`)
/// with support for:
/// - **System clipboard** (`+` register) - OS clipboard integration
/// - **Selection clipboard** (`*` register) - X11 primary selection (Linux)
/// - **Yank history** (registers `0-9`) - Ring buffer of recent yanks
///
/// # Architecture
///
/// Following the hybrid approach:
/// - **Kernel** owns basic registers (`RegisterBank`: unnamed + a-z)
/// - **Driver** defines this trait for extended clipboard features
/// - **Module** implements this trait with actual clipboard access
///
/// # Design Philosophy
///
/// - **Mechanism focus**: Defines WHAT operations are available
/// - **Thread-safe**: All methods take `&self` with internal locking
/// - **Graceful degradation**: Clipboard operations may fail (headless, SSH, etc.)
///
/// # History Registers (0-9)
///
/// Vim's numbered registers work as a ring buffer:
/// - `"0` always contains the most recent yank (NOT delete)
/// - `"1`-`"9` contain the most recent deletes (ring buffer)
///
/// For simplicity, this implementation uses a unified history:
/// - `"0` = most recent yank/delete
/// - `"1`-`"9` = older entries (push-down stack)
///
/// # Example
///
/// ```ignore
/// use reovim_driver_clipboard::ClipboardProvider;
///
/// // After yanking text
/// provider.push_history(content.clone());
///
/// // Copy to system clipboard
/// if let Err(e) = provider.copy_to_clipboard(&content.text) {
///     // Graceful degradation - clipboard may not be available
///     tracing::debug!("Clipboard unavailable: {}", e);
/// }
///
/// // Paste from history
/// if let Some(content) = provider.history_entry(0) {
///     // Use content from "0 register
/// }
/// ```
pub trait ClipboardProvider: Send + Sync {
    // ========================================================================
    // History Support (registers 0-9)
    // ========================================================================

    /// Get the full yank history.
    ///
    /// Returns up to 10 entries (registers 0-9), most recent first.
    fn history(&self) -> Vec<RegisterContent>;

    /// Get a specific history entry by index.
    ///
    /// Index 0 = most recent (`"0`), index 9 = oldest (`"9`).
    /// Returns `None` if the index is out of range or empty.
    fn history_entry(&self, index: usize) -> Option<RegisterContent>;

    /// Push new content to the history.
    ///
    /// This should be called after any yank or delete operation.
    /// The new content becomes `"0`, existing entries shift down.
    fn push_history(&self, content: RegisterContent);

    /// Get the history capacity (typically 10 for registers 0-9).
    fn history_capacity(&self) -> usize {
        10
    }

    /// Get the current history size.
    fn history_len(&self) -> usize;

    // ========================================================================
    // System Clipboard (+ register)
    // ========================================================================

    /// Check if the system clipboard is available.
    ///
    /// Returns `false` in headless environments, SSH without forwarding, etc.
    fn clipboard_available(&self) -> bool;

    /// Copy text to the system clipboard (`"+` register).
    ///
    /// # Errors
    ///
    /// Returns an error if the clipboard is unavailable or the operation fails.
    fn copy_to_clipboard(&self, text: &str) -> Result<(), ClipboardError>;

    /// Paste text from the system clipboard (`"+` register).
    ///
    /// Returns `Ok(None)` if the clipboard is empty.
    ///
    /// # Errors
    ///
    /// Returns an error if the clipboard is unavailable or the operation fails.
    fn paste_from_clipboard(&self) -> Result<Option<String>, ClipboardError>;

    // ========================================================================
    // Selection Clipboard (* register, X11 primary)
    // ========================================================================

    /// Check if the selection clipboard is available.
    ///
    /// On X11, this is the "primary selection" (middle-click paste).
    /// On other platforms, this typically mirrors the system clipboard.
    fn selection_available(&self) -> bool;

    /// Copy text to the selection clipboard (`"*` register).
    ///
    /// # Errors
    ///
    /// Returns an error if the selection is unavailable or the operation fails.
    fn copy_to_selection(&self, text: &str) -> Result<(), ClipboardError>;

    /// Paste text from the selection clipboard (`"*` register).
    ///
    /// Returns `Ok(None)` if the selection is empty.
    ///
    /// # Errors
    ///
    /// Returns an error if the selection is unavailable or the operation fails.
    fn paste_from_selection(&self) -> Result<Option<String>, ClipboardError>;

    // ========================================================================
    // Convenience Methods
    // ========================================================================

    /// Check if any clipboard is available (system or selection).
    fn any_clipboard_available(&self) -> bool {
        self.clipboard_available() || self.selection_available()
    }

    /// Get content for a numbered register (0-9).
    ///
    /// This is a convenience wrapper around `history_entry`.
    fn get_numbered(&self, n: char) -> Option<RegisterContent> {
        if n.is_ascii_digit() {
            let index = (n as u8 - b'0') as usize;
            self.history_entry(index)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Mock clipboard for testing default methods.
    struct MockClipboard {
        available: bool,
    }

    impl ClipboardProvider for MockClipboard {
        fn history(&self) -> Vec<RegisterContent> {
            vec![
                RegisterContent::new("first", reovim_kernel::api::v1::YankType::Characterwise),
                RegisterContent::new("second", reovim_kernel::api::v1::YankType::Characterwise),
            ]
        }
        fn history_entry(&self, index: usize) -> Option<RegisterContent> {
            self.history().get(index).cloned()
        }
        fn push_history(&self, _content: RegisterContent) {}
        fn history_len(&self) -> usize {
            2
        }
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
    fn history_capacity_default() {
        let clip = MockClipboard { available: true };
        assert_eq!(clip.history_capacity(), 10);
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
    fn get_numbered_valid_digit() {
        let clip = MockClipboard { available: true };
        let entry = clip.get_numbered('0');
        assert!(entry.is_some());
        assert_eq!(entry.unwrap().text, "first");
    }

    #[test]
    fn get_numbered_out_of_range() {
        let clip = MockClipboard { available: true };
        assert!(clip.get_numbered('9').is_none());
    }

    #[test]
    fn get_numbered_non_digit() {
        let clip = MockClipboard { available: true };
        assert!(clip.get_numbered('a').is_none());
    }

    #[test]
    fn get_numbered_second_entry() {
        let clip = MockClipboard { available: true };
        let entry = clip.get_numbered('1');
        assert!(entry.is_some());
        assert_eq!(entry.unwrap().text, "second");
    }

    #[test]
    fn history_returns_entries() {
        let clip = MockClipboard { available: true };
        let history = clip.history();
        assert_eq!(history.len(), 2);
        assert_eq!(history[0].text, "first");
        assert_eq!(history[1].text, "second");
    }

    #[test]
    fn history_len_matches() {
        let clip = MockClipboard { available: true };
        assert_eq!(clip.history_len(), 2);
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

    #[test]
    fn history_capacity_custom_override() {
        struct CustomClipboard;
        impl ClipboardProvider for CustomClipboard {
            fn history(&self) -> Vec<RegisterContent> {
                vec![]
            }
            fn history_entry(&self, _index: usize) -> Option<RegisterContent> {
                None
            }
            fn push_history(&self, _content: RegisterContent) {}
            fn history_capacity(&self) -> usize {
                20
            }
            fn history_len(&self) -> usize {
                0
            }
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
                Ok(None)
            }
        }

        let clip = CustomClipboard;
        assert_eq!(clip.history_capacity(), 20);
        // selection_available is true, clipboard_available is false
        assert!(clip.any_clipboard_available());
    }

    #[test]
    fn get_numbered_all_digits() {
        let clip = MockClipboard { available: true };
        // '0' should return "first", '1' should return "second", '2'-'9' None
        assert!(clip.get_numbered('0').is_some());
        assert!(clip.get_numbered('1').is_some());
        for c in '2'..='9' {
            assert!(clip.get_numbered(c).is_none());
        }
    }

    // ========================================================================
    // any_clipboard_available with selection available
    // ========================================================================

    #[test]
    fn any_clipboard_available_with_selection_only() {
        struct SelectionOnlyClipboard;
        impl ClipboardProvider for SelectionOnlyClipboard {
            fn history(&self) -> Vec<RegisterContent> {
                vec![]
            }
            fn history_entry(&self, _index: usize) -> Option<RegisterContent> {
                None
            }
            fn push_history(&self, _content: RegisterContent) {}
            fn history_len(&self) -> usize {
                0
            }
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
    // get_numbered edge cases
    // ========================================================================

    #[test]
    fn get_numbered_non_ascii_digit() {
        let clip = MockClipboard { available: true };
        // Non-ASCII digits should return None
        assert!(clip.get_numbered('z').is_none());
        assert!(clip.get_numbered('@').is_none());
        assert!(clip.get_numbered(' ').is_none());
    }

    #[test]
    fn get_numbered_uppercase_letters() {
        let clip = MockClipboard { available: true };
        assert!(clip.get_numbered('A').is_none());
        assert!(clip.get_numbered('Z').is_none());
    }

    // ========================================================================
    // push_history is callable
    // ========================================================================

    #[test]
    fn push_history_does_not_panic() {
        let clip = MockClipboard { available: true };
        let content =
            RegisterContent::new("test push", reovim_kernel::api::v1::YankType::Characterwise);
        clip.push_history(content);
        // MockClipboard's push_history is a no-op, verify it doesn't panic
    }

    // ========================================================================
    // history_entry out of bounds
    // ========================================================================

    #[test]
    fn history_entry_out_of_bounds() {
        let clip = MockClipboard { available: true };
        assert!(clip.history_entry(10).is_none());
        assert!(clip.history_entry(100).is_none());
        assert!(clip.history_entry(usize::MAX).is_none());
    }

    // ========================================================================
    // Clipboard errors from methods
    // ========================================================================

    #[test]
    fn copy_to_selection_error_message() {
        let clip = MockClipboard { available: true };
        let err = clip.copy_to_selection("text").unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("not available") || msg.contains("no selection"));
    }

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
    fn clipboard_provider_is_object_safe() {
        fn _accepts_ref(_: &dyn ClipboardProvider) {}
        fn _accepts_box(_: Box<dyn ClipboardProvider>) {}
        fn _accepts_arc(_: std::sync::Arc<dyn ClipboardProvider>) {}
    }

    // ========================================================================
    // Paste from clipboard returns None
    // ========================================================================

    #[test]
    fn paste_from_clipboard_empty() {
        struct EmptyClipboard;
        impl ClipboardProvider for EmptyClipboard {
            fn history(&self) -> Vec<RegisterContent> {
                vec![]
            }
            fn history_entry(&self, _index: usize) -> Option<RegisterContent> {
                None
            }
            fn push_history(&self, _content: RegisterContent) {}
            fn history_len(&self) -> usize {
                0
            }
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

    // ========================================================================
    // History with linewise content
    // ========================================================================

    #[test]
    fn push_history_linewise() {
        let clip = MockClipboard { available: true };
        let content = RegisterContent::new("line\n", reovim_kernel::api::v1::YankType::Linewise);
        clip.push_history(content);
    }

    // ========================================================================
    // Default history_capacity
    // ========================================================================

    #[test]
    fn history_capacity_default_is_ten() {
        // Verify multiple implementations get default of 10
        let clip = MockClipboard { available: false };
        assert_eq!(clip.history_capacity(), 10);
    }
}
