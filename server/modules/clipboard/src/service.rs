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
#[path = "service_tests.rs"]
mod tests;
