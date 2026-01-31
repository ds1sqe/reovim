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
}
