//! Built-in theme variants.
//!
//! This is a pure-data enum; the `Style`-aware palette tables live in
//! the display tier (`reovim-driver-display::style::builtin`). The
//! [`BuiltinTheme::load`] helper delegates to the registered
//! [`ThemeFactory`] when present and otherwise falls back to a stub
//! provider that reports only the theme name.
//!
//! [`ThemeFactory`]: super::ThemeFactory

use std::{any::Any, sync::Arc};

use super::{factory::theme_factory, provider::ThemeProvider};

/// Built-in theme variants.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum BuiltinTheme {
    /// Dark theme (`OneDark`-inspired).
    #[default]
    Dark,
    /// Light theme.
    Light,
    /// Tokyo Night Orange variant.
    TokyoNightOrange,
}

impl BuiltinTheme {
    /// Get the theme name.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Dark => "dark",
            Self::Light => "light",
            Self::TokyoNightOrange => "tokyo-night-orange",
        }
    }

    /// All available built-in theme variants.
    #[must_use]
    pub const fn all() -> &'static [Self] {
        &[Self::Dark, Self::Light, Self::TokyoNightOrange]
    }

    /// Load this theme as an `Arc<dyn ThemeProvider>`.
    ///
    /// If a [`ThemeFactory`] has been installed (typically by
    /// `reovim_driver_display::install_theme_factory()` during process
    /// startup), the factory constructs a `Style`-aware concrete
    /// theme. If no factory is installed — for example in pure-server
    /// or subprocess deployments where no client renderer links the
    /// display crate — this returns a stub provider that reports only
    /// the theme name. The server tier never inspects styles, so the
    /// stub is functionally adequate.
    ///
    /// [`ThemeFactory`]: super::ThemeFactory
    #[must_use]
    pub fn load(self) -> Arc<dyn ThemeProvider> {
        theme_factory().map_or_else(
            || Arc::new(StubBuiltinTheme { variant: self }) as Arc<dyn ThemeProvider>,
            |factory| factory.load_builtin(self),
        )
    }
}

/// Fallback theme provider used when no factory is registered.
///
/// Reports only its name; calls into the display side that try to
/// downcast it to a `Style`-aware view will see a no-op.
struct StubBuiltinTheme {
    variant: BuiltinTheme,
}

impl ThemeProvider for StubBuiltinTheme {
    fn name(&self) -> &str {
        self.variant.name()
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

#[cfg(test)]
#[path = "builtin_tests.rs"]
mod tests;
