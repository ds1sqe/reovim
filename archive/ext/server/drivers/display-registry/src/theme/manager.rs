//! Slim theme manager (server-tier).
//!
//! Holds the current `Arc<dyn ThemeProvider>` and exposes name-only
//! accessors. The display tier wraps this manager (or extends it via
//! a trait) to add `Style`-aware lookups; this crate stays
//! display-primitive-free.

use std::sync::Arc;

use reovim_arch::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};

use super::provider::ThemeProvider;

/// Slim theme manager — owns the current theme provider.
///
/// The display tier registers `Style`-aware concrete providers via
/// the [`ThemeFactory`] indirection. This manager exposes only the
/// theme name and the boxed provider; rendering code on the display
/// side recovers the styled view via `AsAny` downcast on
/// [`current_theme`].
///
/// [`ThemeFactory`]: super::ThemeFactory
/// [`current_theme`]: ThemeManager::current_theme
pub struct ThemeManager {
    current: Arc<dyn ThemeProvider>,
}

impl ThemeManager {
    /// Create a new theme manager wrapping the given provider.
    #[must_use]
    pub fn new(theme: Arc<dyn ThemeProvider>) -> Self {
        Self { current: theme }
    }

    /// Replace the current theme provider.
    pub fn set_theme(&mut self, theme: Arc<dyn ThemeProvider>) {
        self.current = theme;
    }

    /// Borrow the current theme provider.
    #[must_use]
    pub const fn current_theme(&self) -> &Arc<dyn ThemeProvider> {
        &self.current
    }

    /// Get the current theme name.
    #[must_use]
    pub fn current_theme_name(&self) -> &str {
        self.current.name()
    }
}

impl reovim_kernel::api::v1::Service for ThemeManager {}

/// Thread-safe wrapper around [`ThemeManager`] for `ServiceRegistry`.
///
/// The `RwLock` lets multiple readers query the current theme name
/// while writers (e.g. the `:colorscheme` command) swap it in.
///
/// # Example
///
/// ```ignore
/// use std::sync::Arc;
/// use reovim_driver_display_registry::theme::{
///     BuiltinTheme, SharedThemeManager,
/// };
///
/// let manager = SharedThemeManager::new(BuiltinTheme::Dark.load());
/// services.register(Arc::new(manager));
///
/// // Later:
/// let shared = services.get::<SharedThemeManager>().unwrap();
/// let _name = shared.read().current_theme_name();
/// shared.write().set_theme(BuiltinTheme::Light.load());
/// ```
pub struct SharedThemeManager(RwLock<ThemeManager>);

impl SharedThemeManager {
    /// Create a new shared theme manager.
    #[must_use]
    pub fn new(theme: Arc<dyn ThemeProvider>) -> Self {
        Self(RwLock::new(ThemeManager::new(theme)))
    }

    /// Acquire a read lock on the inner manager.
    pub fn read(&self) -> RwLockReadGuard<'_, ThemeManager> {
        self.0.read()
    }

    /// Acquire a write lock on the inner manager.
    pub fn write(&self) -> RwLockWriteGuard<'_, ThemeManager> {
        self.0.write()
    }
}

impl reovim_kernel::api::v1::Service for SharedThemeManager {}

#[cfg(test)]
#[path = "manager_tests.rs"]
mod tests;
