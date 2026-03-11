//! Clipboard service implementation.
//!
//! Implements `ClipboardProvider` trait with system clipboard access.
//!
//! # Architecture (#515 Phase 4)
//!
//! History (numbered registers 0-9) has been moved to per-client `HistoryRing`
//! in `EditingState`. This service now handles only OS clipboard I/O.

use {
    reovim_arch::sync::RwLock,
    reovim_driver_clipboard::{ClipboardError, ClipboardProvider},
};

/// Clipboard service implementing `ClipboardProvider`.
///
/// Provides:
/// - System clipboard access (`+` register) via `arboard`
/// - Selection clipboard access (`*` register) - mirrors system clipboard on non-X11
///
/// # Thread Safety
///
/// Uses interior mutability with `RwLock` for thread-safe access.
/// The arboard clipboard is created lazily and cached.
pub struct ClipboardService {
    /// Cached clipboard instance.
    clipboard: RwLock<Option<arboard::Clipboard>>,
}

impl ClipboardService {
    /// Create a new clipboard service.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            clipboard: RwLock::new(None),
        }
    }

    /// Get or create the clipboard instance.
    #[cfg_attr(coverage_nightly, coverage(off))]
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
    // System Clipboard (+ register)
    // ========================================================================

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn clipboard_available(&self) -> bool {
        // Try to get or create clipboard
        self.with_clipboard(|_| Ok(())).is_ok()
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn copy_to_clipboard(&self, text: &str) -> Result<(), ClipboardError> {
        self.with_clipboard(|clip| {
            clip.set_text(text)
                .map_err(|e| ClipboardError::WriteFailed(e.to_string()))
        })
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
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

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn selection_available(&self) -> bool {
        // On non-X11 platforms, selection mirrors the system clipboard
        self.clipboard_available()
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn copy_to_selection(&self, text: &str) -> Result<(), ClipboardError> {
        // On non-X11 platforms, selection mirrors the system clipboard
        // On X11, arboard handles primary selection automatically when available
        self.copy_to_clipboard(text)
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn paste_from_selection(&self) -> Result<Option<String>, ClipboardError> {
        // On non-X11 platforms, selection mirrors the system clipboard
        self.paste_from_clipboard()
    }
}

impl std::fmt::Debug for ClipboardService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ClipboardService")
            .field("clipboard_available", &self.clipboard_available())
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Note: System clipboard tests are skipped in CI as they require a display
    // Run locally with: cargo test -p reovim-module-clipboard -- --ignored
    #[test]
    #[ignore = "requires display for clipboard access (#576)"]
    #[cfg_attr(coverage_nightly, coverage(off))]
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
        // Both start with no state - just verify they don't panic
        let _from_new = ClipboardService::new();
        let _from_default = ClipboardService::default();
    }

    #[test]
    fn test_debug_impl() {
        let service = ClipboardService::new();
        let debug = format!("{service:?}");
        assert!(debug.contains("ClipboardService"));
        assert!(debug.contains("clipboard_available"));
    }

    // System clipboard tests - exercise the delegation paths.
    // These may fail in CI without a display, but we test the code paths
    // that don't depend on clipboard availability.

    #[test]
    fn test_selection_available_delegates_to_clipboard() {
        let service = ClipboardService::new();
        assert_eq!(service.selection_available(), service.clipboard_available());
    }

    #[test]
    fn test_clipboard_available_consistent() {
        let service = ClipboardService::new();
        let first = service.clipboard_available();
        let second = service.clipboard_available();
        assert_eq!(first, second);
    }

    #[test]
    fn test_copy_to_selection_when_no_display() {
        let service = ClipboardService::new();
        let _ = service.copy_to_selection("test");
    }

    #[test]
    fn test_paste_from_selection_when_no_display() {
        let service = ClipboardService::new();
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
    fn test_clipboard_available_called_multiple_times() {
        let service = ClipboardService::new();
        for _ in 0..5 {
            let _ = service.clipboard_available();
        }
    }

    #[test]
    fn test_selection_available_matches_clipboard() {
        let service = ClipboardService::new();
        let clip = service.clipboard_available();
        let sel = service.selection_available();
        assert_eq!(clip, sel);
    }

    #[test]
    fn test_any_clipboard_available() {
        let service = ClipboardService::new();
        // any_clipboard_available = clipboard || selection
        // Both delegate to same underlying, so they agree
        let any = service.any_clipboard_available();
        assert_eq!(any, service.clipboard_available());
    }
}
