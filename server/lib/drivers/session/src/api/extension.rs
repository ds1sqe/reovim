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
//! use reovim_driver_session::api::ExtensionApi;
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
pub trait ExtensionApi: Send {
    /// Get extension by type (immutable).
    ///
    /// Returns `None` if the extension hasn't been created yet.
    fn ext<T: SessionExtension>(&self) -> Option<&T>;

    /// Get extension by type (mutable), creating if needed.
    ///
    /// If the extension doesn't exist, it's created using `T::create()`.
    /// This is the primary way modules access their state.
    fn ext_mut<T: SessionExtension>(&mut self) -> &mut T;
}

#[cfg(test)]
mod tests {
    // ExtensionApi trait object safety is not required since it uses generics.
    // Tests for extension functionality are in the runtime module.
}
