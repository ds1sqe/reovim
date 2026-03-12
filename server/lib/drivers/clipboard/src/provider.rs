//! Clipboard provider trait.
//!
//! Provides OS clipboard access (system clipboard and X11 primary selection).
//!
//! # Architecture (#515 Phase 4)
//!
//! History (numbered registers 0-9) is now per-client via `HistoryRing` in
//! `EditingState`, managed by `SessionRuntime::push_to_clipboard_history`.
//! This trait is purely for OS clipboard I/O.

use crate::ClipboardError;

/// OS clipboard provider interface.
///
/// Provides system clipboard (`+` register) and selection clipboard (`*` register)
/// access. Thread-safe with graceful degradation for headless/SSH environments.
///
/// # Architecture (#515 Phase 4)
///
/// - **System clipboard** (`+`) - OS clipboard integration
/// - **Selection clipboard** (`*`) - X11 primary selection (Linux)
/// - **History** (0-9) - Moved to per-client `HistoryRing` in `EditingState`
///
/// # Example
///
/// ```ignore
/// use reovim_driver_clipboard::ClipboardProvider;
///
/// // Copy to system clipboard
/// if let Err(e) = provider.copy_to_clipboard(&content.text) {
///     // Graceful degradation - clipboard may not be available
///     tracing::debug!("Clipboard unavailable: {}", e);
/// }
/// ```
pub trait ClipboardProvider: Send + Sync {
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
}

#[cfg(test)]
#[path = "provider_tests.rs"]
mod tests;
