//! Clipboard access for system clipboard operations.
//!
//! This module provides the [`ClipboardApi`] trait for direct system clipboard I/O.
//! Commands and operators use this to explicitly interact with the OS clipboard.
//!
//! # Design (#515)
//!
//! Decoupled from [`RegisterApi`](super::RegisterApi): registers (`a-z`, `""`, `0-9`)
//! are pure per-client storage. Clipboard (`+`, `*`) is a separate shared OS resource
//! accessed through this trait.
//!
//! # Example
//!
//! ```ignore
//! use reovim_driver_session::api::{ClipboardApi, RegisterApi, RegisterContent};
//!
//! fn yank_to_clipboard<S: RegisterApi + ClipboardApi>(
//!     session: &mut S,
//!     register: Option<char>,
//!     text: &str,
//! ) {
//!     let content = RegisterContent::characterwise(text);
//!     session.set_register(register, content);
//!
//!     // Explicitly sync to OS clipboard for + and *
//!     match register {
//!         Some('+') => { session.copy_to_clipboard(text); }
//!         Some('*') => { session.copy_to_selection(text); }
//!         _ => {}
//!     }
//! }
//! ```

/// System clipboard access for copy/paste operations.
///
/// Provides direct access to the OS clipboard (`+` register) and
/// selection clipboard (`*` register, X11 primary selection).
///
/// # Graceful Degradation
///
/// All methods return `bool`/`Option` rather than `Result` because
/// clipboard unavailability (headless, SSH, Wayland without clipboard
/// manager) is a normal condition, not an error.
///
/// # Shared Resource
///
/// The clipboard is an OS-level shared resource. All clients in a session
/// share the same clipboard. Per-client register storage (`+`/`*` in
/// `RegisterBank`) provides a local fallback when the clipboard is
/// unavailable.
pub trait ClipboardApi: Send {
    /// Copy text to the system clipboard (`+` register).
    ///
    /// Returns `true` if the copy succeeded, `false` if the clipboard
    /// is unavailable or the operation failed.
    fn copy_to_clipboard(&self, text: &str) -> bool;

    /// Paste text from the system clipboard (`+` register).
    ///
    /// Returns `None` if the clipboard is unavailable or empty.
    fn paste_from_clipboard(&self) -> Option<String>;

    /// Copy text to the selection clipboard (`*` register).
    ///
    /// On X11, this is the primary selection (middle-click paste).
    /// On other platforms, this typically mirrors the system clipboard.
    ///
    /// Returns `true` if the copy succeeded.
    fn copy_to_selection(&self, text: &str) -> bool;

    /// Paste text from the selection clipboard (`*` register).
    ///
    /// Returns `None` if the selection is unavailable or empty.
    fn paste_from_selection(&self) -> Option<String>;
}
#[cfg(test)]
#[path = "tests/clipboard.rs"]
mod tests;
