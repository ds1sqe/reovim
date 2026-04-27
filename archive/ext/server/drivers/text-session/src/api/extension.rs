//! Module extension access.
//!
//! This module provides the [`ExtensionApi`] trait for accessing
//! module-specific per-session state.
//!
//! # Design
//!
//! Modules store per-session policy state via `SessionExtension`.
//! `ExtensionApi` provides type-safe access to that state.
//!
//! # Example
//!
//! ```ignore
//! use reovim_driver_text_session::api::ExtensionApi;
//!
//! // Access vim-specific state in a resolver
//! fn get_pending_count<S: ExtensionApi>(session: &mut S) -> Option<usize> {
//!     let vim = session.ext_mut::<VimSessionState>();
//!     vim.pending_count
//! }
//! ```

use crate::SessionExtension;

/// Module extension access.
///
/// Allows resolvers and commands to access module-specific per-session state.
/// Extensions are lazily created on first access via `ext_mut`.
///
/// Per-client extensions (`ext` / `ext_mut`) are always available. Shared
/// (session-wide) extensions (`shared_ext` / `shared_ext_mut`) are optional
/// and return `None` by default — implementations that have access to
/// `AppState.extensions` override these methods (#543).
pub trait ExtensionApi: Send {
    /// Get per-client extension by type (immutable).
    ///
    /// Returns `None` if the extension hasn't been created yet.
    fn ext<T: SessionExtension>(&self) -> Option<&T>;

    /// Get per-client extension by type (mutable), creating if needed.
    ///
    /// If the extension doesn't exist, it's created using `T::create()`.
    /// This is the primary way modules access their state.
    fn ext_mut<T: SessionExtension>(&mut self) -> &mut T;

    /// Get shared (session-wide) extension by type (immutable).
    ///
    /// Returns `None` if the extension hasn't been created or shared
    /// extensions are not available (e.g., test stubs).
    ///
    /// Shared extensions live in `AppState.extensions` and are visible
    /// to all clients in a session. Used for cross-client coordination
    /// (e.g., multiplayer match state).
    fn shared_ext<T: SessionExtension>(&self) -> Option<&T> {
        let _ = std::marker::PhantomData::<T>;
        None
    }

    /// Get shared (session-wide) extension by type (mutable), creating if needed.
    ///
    /// Returns `None` if shared extensions are not available.
    fn shared_ext_mut<T: SessionExtension>(&mut self) -> Option<&mut T> {
        let _ = std::marker::PhantomData::<T>;
        None
    }
}
#[cfg(test)]
#[path = "tests/extension.rs"]
mod tests;
