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
