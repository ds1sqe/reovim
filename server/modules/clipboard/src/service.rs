//! Clipboard service implementation.
//!
//! Implements `ClipboardProvider` trait with system clipboard access and history.

use {
    crate::HistoryRing,
    reovim_arch::sync::RwLock,
    reovim_driver_clipboard::{ClipboardError, ClipboardProvider},
    reovim_kernel::api::v1::RegisterContent,
};

/// Clipboard service implementing `ClipboardProvider`.
///
/// Provides:
/// - System clipboard access (`+` register) via `arboard`
/// - Selection clipboard access (`*` register) - mirrors system clipboard on non-X11
/// - Yank history (registers 0-9)
///
/// # Thread Safety
///
/// Uses interior mutability with `RwLock` for thread-safe access.
/// The arboard clipboard is created lazily and cached.
pub struct ClipboardService {
    /// Yank history (registers 0-9).
    history: RwLock<HistoryRing>,
    /// Cached clipboard instance.
    clipboard: RwLock<Option<arboard::Clipboard>>,
}

impl ClipboardService {
    /// Create a new clipboard service.
    #[must_use]
    pub fn new() -> Self {
        Self {
            history: RwLock::new(HistoryRing::default()),
            clipboard: RwLock::new(None),
        }
    }

    /// Get or create the clipboard instance.
    fn with_clipboard<F, R>(&self, f: F) -> Result<R, ClipboardError>
    where
        F: FnOnce(&mut arboard::Clipboard) -> Result<R, ClipboardError>,
    {
        let mut guard = self.clipboard.write();

        // Lazily initialize clipboard
        if guard.is_none() {
            match arboard::Clipboard::new() {
                Ok(clip) => *guard = Some(clip),
                Err(e) => {
                    return Err(ClipboardError::NotAvailable(format!(
                        "Failed to initialize clipboard: {e}"
                    )));
                }
            }
        }

        guard.as_mut().map_or_else(
            || Err(ClipboardError::NotAvailable("Clipboard not initialized".to_string())),
            f,
        )
    }
}

impl Default for ClipboardService {
    fn default() -> Self {
        Self::new()
    }
}

impl ClipboardProvider for ClipboardService {
    // ========================================================================
    // History Support (registers 0-9)
    // ========================================================================

    fn history(&self) -> Vec<RegisterContent> {
        self.history.read().all().to_vec()
    }

    fn history_entry(&self, index: usize) -> Option<RegisterContent> {
        self.history.read().get(index).cloned()
    }

    fn push_history(&self, content: RegisterContent) {
        self.history.write().push(content);
    }

    fn history_len(&self) -> usize {
        self.history.read().len()
    }

    // ========================================================================
    // System Clipboard (+ register)
    // ========================================================================

    fn clipboard_available(&self) -> bool {
        // Try to get or create clipboard
        self.with_clipboard(|_| Ok(())).is_ok()
    }

    fn copy_to_clipboard(&self, text: &str) -> Result<(), ClipboardError> {
        self.with_clipboard(|clip| {
            clip.set_text(text)
                .map_err(|e| ClipboardError::WriteFailed(e.to_string()))
        })
    }

    fn paste_from_clipboard(&self) -> Result<Option<String>, ClipboardError> {
        self.with_clipboard(|clip| match clip.get_text() {
            Ok(text) => {
                if text.is_empty() {
                    Ok(None)
                } else {
                    Ok(Some(text))
                }
            }
            Err(arboard::Error::ContentNotAvailable) => Ok(None),
            Err(e) => Err(ClipboardError::ReadFailed(e.to_string())),
        })
    }

    // ========================================================================
    // Selection Clipboard (* register)
    // ========================================================================

    fn selection_available(&self) -> bool {
        // On non-X11 platforms, selection mirrors the system clipboard
        self.clipboard_available()
    }

    fn copy_to_selection(&self, text: &str) -> Result<(), ClipboardError> {
        // On non-X11 platforms, selection mirrors the system clipboard
        // On X11, arboard handles primary selection automatically when available
        self.copy_to_clipboard(text)
    }

    fn paste_from_selection(&self) -> Result<Option<String>, ClipboardError> {
        // On non-X11 platforms, selection mirrors the system clipboard
        self.paste_from_clipboard()
    }
}

impl std::fmt::Debug for ClipboardService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ClipboardService")
            .field("history_len", &self.history.read().len())
            .field("clipboard_available", &self.clipboard_available())
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use {super::*, reovim_kernel::api::v1::YankType};

    fn content(text: &str) -> RegisterContent {
        RegisterContent::new(text.to_string(), YankType::Characterwise)
    }

    #[test]
    fn test_history() {
        let service = ClipboardService::new();

        service.push_history(content("first"));
        service.push_history(content("second"));

        assert_eq!(service.history_len(), 2);
        assert_eq!(service.history_entry(0).map(|c| c.text), Some("second".to_string()));
        assert_eq!(service.history_entry(1).map(|c| c.text), Some("first".to_string()));
    }

    #[test]
    fn test_get_numbered() {
        let service = ClipboardService::new();

        service.push_history(content("zero"));
        service.push_history(content("one"));

        // Using convenience method from trait
        assert_eq!(service.get_numbered('0').map(|c| c.text), Some("one".to_string()));
        assert_eq!(service.get_numbered('1').map(|c| c.text), Some("zero".to_string()));
        assert!(service.get_numbered('a').is_none()); // Non-digit
    }

    // Note: System clipboard tests are skipped in CI as they require a display
    // Run locally with: cargo test -p reovim-module-clipboard -- --ignored
    #[test]
    #[ignore = "requires display for clipboard access"]
    fn test_system_clipboard() {
        let service = ClipboardService::new();

        if service.clipboard_available() {
            let test_text = "reovim clipboard test";
            service.copy_to_clipboard(test_text).unwrap();
            let pasted = service.paste_from_clipboard().unwrap();
            assert_eq!(pasted, Some(test_text.to_string()));
        }
    }

    #[test]
    fn test_default_creates_same_as_new() {
        let from_new = ClipboardService::new();
        let from_default = ClipboardService::default();
        // Both start with empty history
        assert_eq!(from_new.history_len(), 0);
        assert_eq!(from_default.history_len(), 0);
    }

    #[test]
    fn test_history_returns_all_entries() {
        let service = ClipboardService::new();
        service.push_history(content("a"));
        service.push_history(content("b"));
        service.push_history(content("c"));

        let history = service.history();
        assert_eq!(history.len(), 3);
        assert_eq!(history[0].text, "c");
        assert_eq!(history[1].text, "b");
        assert_eq!(history[2].text, "a");
    }

    #[test]
    fn test_history_entry_out_of_bounds() {
        let service = ClipboardService::new();
        service.push_history(content("only"));

        assert!(service.history_entry(0).is_some());
        assert!(service.history_entry(1).is_none());
        assert!(service.history_entry(100).is_none());
    }

    #[test]
    fn test_empty_history() {
        let service = ClipboardService::new();
        assert_eq!(service.history_len(), 0);
        assert!(service.history().is_empty());
        assert!(service.history_entry(0).is_none());
    }

    #[test]
    fn test_get_numbered_all_digits() {
        let service = ClipboardService::new();

        // Push 10 items to fill registers 0-9
        for i in 0..10 {
            service.push_history(content(&format!("item{i}")));
        }

        // Register '0' is the most recent (item9)
        assert_eq!(service.get_numbered('0').map(|c| c.text), Some("item9".to_string()));

        // Register '9' is the oldest (item0)
        assert_eq!(service.get_numbered('9').map(|c| c.text), Some("item0".to_string()));
    }

    #[test]
    fn test_debug_impl() {
        let service = ClipboardService::new();
        service.push_history(content("test"));
        // Just verify Debug impl does not panic
        let debug = format!("{service:?}");
        assert!(debug.contains("ClipboardService"));
        assert!(debug.contains("history_len"));
    }

    #[test]
    fn test_history_preserves_yank_type() {
        let service = ClipboardService::new();
        service.push_history(RegisterContent::new("line content".to_string(), YankType::Linewise));

        let entry = service.history_entry(0).unwrap();
        assert_eq!(entry.yank_type, YankType::Linewise);
        assert_eq!(entry.text, "line content");
    }

    // System clipboard tests - exercise the delegation paths.
    // These may fail in CI without a display, but we test the code paths
    // that don't depend on clipboard availability.

    #[test]
    fn test_selection_available_delegates_to_clipboard() {
        let service = ClipboardService::new();
        // selection_available delegates to clipboard_available
        // Both should return the same value
        assert_eq!(service.selection_available(), service.clipboard_available());
    }

    #[test]
    fn test_clipboard_available_consistent() {
        let service = ClipboardService::new();
        // Call twice - should be consistent (caches the clipboard)
        let first = service.clipboard_available();
        let second = service.clipboard_available();
        assert_eq!(first, second);
    }

    #[test]
    fn test_copy_to_selection_when_no_display() {
        let service = ClipboardService::new();
        // In CI without display, this should return an error
        // but should not panic
        let _ = service.copy_to_selection("test");
    }

    #[test]
    fn test_paste_from_selection_when_no_display() {
        let service = ClipboardService::new();
        // In CI without display, this should return an error
        // but should not panic
        let _ = service.paste_from_selection();
    }

    #[test]
    fn test_copy_to_clipboard_when_no_display() {
        let service = ClipboardService::new();
        let _ = service.copy_to_clipboard("test");
    }

    #[test]
    fn test_paste_from_clipboard_when_no_display() {
        let service = ClipboardService::new();
        let _ = service.paste_from_clipboard();
    }

    #[test]
    fn test_debug_impl_with_history() {
        let service = ClipboardService::new();
        service.push_history(content("one"));
        service.push_history(content("two"));
        service.push_history(content("three"));

        let debug = format!("{service:?}");
        assert!(debug.contains("ClipboardService"));
        assert!(debug.contains("history_len"));
        assert!(debug.contains("clipboard_available"));
    }

    #[test]
    fn test_history_ring_overflow() {
        let service = ClipboardService::new();
        // Push more than 10 items to test ring buffer behavior
        for i in 0..15 {
            service.push_history(content(&format!("item{i}")));
        }

        // History ring should cap at 10 entries
        assert!(service.history_len() <= 10);
    }

    #[test]
    fn test_get_numbered_with_non_digit_chars() {
        let service = ClipboardService::new();
        service.push_history(content("test"));

        // Various non-digit characters should return None
        assert!(service.get_numbered('a').is_none());
        assert!(service.get_numbered('z').is_none());
        assert!(service.get_numbered('!').is_none());
        assert!(service.get_numbered(' ').is_none());
        assert!(service.get_numbered('-').is_none());
    }

    #[test]
    fn test_get_numbered_empty_history() {
        let service = ClipboardService::new();
        // Even with valid digit, empty history returns None
        assert!(service.get_numbered('0').is_none());
        assert!(service.get_numbered('9').is_none());
    }

    #[test]
    fn test_history_preserves_characterwise_type() {
        let service = ClipboardService::new();
        service
            .push_history(RegisterContent::new("char text".to_string(), YankType::Characterwise));

        let entry = service.history_entry(0).unwrap();
        assert_eq!(entry.yank_type, YankType::Characterwise);
    }

    #[test]
    fn test_clipboard_available_called_multiple_times() {
        let service = ClipboardService::new();
        // Call many times - should not panic or change behavior
        for _ in 0..5 {
            let _ = service.clipboard_available();
        }
    }

    #[test]
    fn test_selection_available_matches_clipboard() {
        let service = ClipboardService::new();
        // selection_available should always match clipboard_available
        let clip = service.clipboard_available();
        let sel = service.selection_available();
        assert_eq!(clip, sel);
    }

    #[test]
    fn test_history_order_is_lifo() {
        let service = ClipboardService::new();
        service.push_history(content("first"));
        service.push_history(content("second"));
        service.push_history(content("third"));

        // Most recent is index 0
        assert_eq!(service.history_entry(0).unwrap().text, "third");
        assert_eq!(service.history_entry(1).unwrap().text, "second");
        assert_eq!(service.history_entry(2).unwrap().text, "first");
    }
}
