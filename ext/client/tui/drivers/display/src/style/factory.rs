//! Display-tier theme factory.
//!
//! [`DisplayThemeFactory`] is the [`ThemeFactory`] implementation
//! that constructs `Style`-aware concrete theme providers
//! (`SimpleBuiltinTheme`, `FileTheme`) on behalf of the registry
//! crate. [`install_theme_factory`] registers the factory in the
//! process-global slot owned by
//! `reovim-driver-display-registry::theme::set_theme_factory`.
//!
//! # Lifecycle
//!
//! Call [`install_theme_factory`] once during process startup, before
//! any code path expects `BuiltinTheme::load()` /
//! `ThemeLoader::load(name)` to return a `Style`-aware provider. In
//! reovim's embedded-default deployment that point is
//! `clients/tui/src/app.rs::App::new`, immediately before the TUI
//! constructs its parallel `ThemeManager::new(BuiltinTheme::Dark.load())`.
//!
//! Calling more than once is a no-op (`OnceLock` semantics).
//!
//! [`ThemeFactory`]: reovim_driver_display_registry::theme::ThemeFactory

use std::sync::Arc;

use reovim_driver_display_registry::theme::{
    BuiltinTheme, ThemeError, ThemeFactory, ThemeProvider, set_theme_factory,
};

use super::{file::FileTheme, theme::SimpleBuiltinTheme};

/// Display-tier theme factory: turns registry-side theme requests
/// into `Style`-aware concrete providers.
///
/// `DisplayThemeFactory` is stateless; one instance per process is
/// sufficient.
#[derive(Debug, Default, Clone, Copy)]
pub struct DisplayThemeFactory;

impl ThemeFactory for DisplayThemeFactory {
    fn load_builtin(&self, theme: BuiltinTheme) -> Arc<dyn ThemeProvider> {
        SimpleBuiltinTheme::into_arc(theme)
    }

    fn load_file(&self, _name: &str, content: &str) -> Result<Arc<dyn ThemeProvider>, ThemeError> {
        let theme = FileTheme::parse(content)?;
        Ok(theme.into_arc())
    }
}

/// Install [`DisplayThemeFactory`] in the registry crate's
/// process-global factory slot.
///
/// Idempotent: a second call is silently dropped (`OnceLock`
/// semantics). Call this once during process startup, before any
/// code reaches `BuiltinTheme::load()` for a `Style`-aware result.
pub fn install_theme_factory() {
    set_theme_factory(Arc::new(DisplayThemeFactory));
}

#[cfg(test)]
#[path = "factory_tests.rs"]
mod tests;
