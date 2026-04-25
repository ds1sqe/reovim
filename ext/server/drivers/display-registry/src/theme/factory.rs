//! Theme factory indirection.
//!
//! The registry crate cannot construct concrete [`ThemeProvider`]
//! instances on its own — the only impls live in the display tier
//! (`reovim-driver-display`) where `Style` lives. To avoid taking a
//! compile-time dep on the display crate, the registry exposes a
//! [`ThemeFactory`] trait that the display tier registers at startup.
//!
//! # Lifecycle
//!
//! 1. Display tier calls [`set_theme_factory`] with
//!    `Arc::new(DisplayThemeFactory)` early in process startup
//!    (e.g. from `clients/tui/src/app.rs::App::new`).
//! 2. Server / module code calls [`BuiltinTheme::load`] or
//!    [`ThemeLoader::load`]; both delegate to the registered factory.
//! 3. If no factory is registered (e.g. server-only / subprocess
//!    deployments where no client renderer is linked) loaders return
//!    stub [`ThemeProvider`] instances that report only a name. The
//!    server tier never inspects styles, so a stub is sufficient.
//!
//! [`BuiltinTheme::load`]: super::BuiltinTheme::load
//! [`ThemeLoader::load`]: super::ThemeLoader::load

use std::sync::{Arc, OnceLock};

use super::{BuiltinTheme, ThemeError, provider::ThemeProvider};

/// Display-tier hook that constructs concrete `ThemeProvider`
/// instances for the registry tier.
pub trait ThemeFactory: Send + Sync {
    /// Construct a built-in theme provider for the given variant.
    fn load_builtin(&self, theme: BuiltinTheme) -> Arc<dyn ThemeProvider>;

    /// Parse raw theme file content (TOML on the display side) and
    /// construct a `ThemeProvider`.
    ///
    /// The `name` argument is the file stem the loader resolved; the
    /// factory is free to override it with the parsed `[meta] name`.
    ///
    /// # Errors
    ///
    /// Returns [`ThemeError`] if parsing or color resolution fails.
    /// Parser-native errors are wrapped into [`ThemeError::Parse`].
    fn load_file(&self, name: &str, content: &str) -> Result<Arc<dyn ThemeProvider>, ThemeError>;
}

static FACTORY: OnceLock<Arc<dyn ThemeFactory>> = OnceLock::new();

/// Install the process-wide theme factory.
///
/// Calling this more than once is a no-op: the first call wins and
/// subsequent calls are silently dropped. This matches the
/// embedded-default deployment, where the display tier installs the
/// factory once on the way up.
pub fn set_theme_factory(factory: Arc<dyn ThemeFactory>) {
    let _ = FACTORY.set(factory);
}

/// Read the currently-installed theme factory, if any.
///
/// Safe to call before [`set_theme_factory`]; returns `None` until a
/// factory is registered.
#[must_use]
pub fn theme_factory() -> Option<Arc<dyn ThemeFactory>> {
    FACTORY.get().cloned()
}

#[cfg(test)]
#[path = "factory_tests.rs"]
mod tests;
