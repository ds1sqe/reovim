//! Display-tier theme extensions.
//!
//! [`StyledTheme`] is the `Style`-aware super-trait that the display
//! tier layers on top of the registry's slim [`ThemeProvider`]. The
//! display crate's concrete theme implementations (`SimpleBuiltinTheme`,
//! `FileTheme`) implement BOTH traits; the registry-side `ThemeManager`
//! holds them as `Arc<dyn ThemeProvider>` and the display side recovers
//! the styled view via `Any` downcast — see
//! [`StyledThemeManagerExt`](super::StyledThemeManagerExt).
//!
//! [`ThemeProvider`]: reovim_driver_display_registry::theme::ThemeProvider

use std::{any::Any, sync::Arc};

use reovim_driver_display_registry::theme::ThemeProvider;

use crate::highlight::Style;

/// Display-tier `Style`-aware view of a [`ThemeProvider`].
///
/// Concrete theme types (`SimpleBuiltinTheme`, `FileTheme`) implement
/// both `ThemeProvider` (the registry-tier slim trait, exposing only
/// the theme name) and `StyledTheme` (this trait, exposing
/// `get_style`). Code that holds an `Arc<dyn ThemeProvider>` recovers
/// the styled view by downcasting via `AsAny`; see
/// [`StyledThemeManagerExt::current_styled_theme`].
///
/// [`StyledThemeManagerExt::current_styled_theme`]:
///     super::StyledThemeManagerExt::current_styled_theme
pub trait StyledTheme: ThemeProvider {
    /// Get the style for a highlight group by name.
    ///
    /// Returns `None` if the group is not defined in this theme.
    fn get_style(&self, group: &str) -> Option<Style>;

    /// Get the default/fallback style.
    fn default_style(&self) -> Style {
        Style::default()
    }
}

/// Simple implementation of built-in themes.
///
/// Theme color tables live in [`super::builtin`] as
/// `LazyLock<HashMap<&'static str, Style>>` palettes; this struct is
/// the concrete provider that consults them. Construction goes
/// through [`super::factory::DisplayThemeFactory::load_builtin`] so
/// the registry crate doesn't see the `Style` type.
pub(super) struct SimpleBuiltinTheme {
    pub(super) variant: super::BuiltinTheme,
}

impl SimpleBuiltinTheme {
    pub(super) fn into_arc(variant: super::BuiltinTheme) -> Arc<dyn ThemeProvider> {
        Arc::new(Self { variant })
    }
}

impl ThemeProvider for SimpleBuiltinTheme {
    fn name(&self) -> &str {
        self.variant.name()
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl StyledTheme for SimpleBuiltinTheme {
    fn get_style(&self, group: &str) -> Option<Style> {
        super::builtin::get_palette(self.variant)
            .get(group)
            .cloned()
    }

    fn default_style(&self) -> Style {
        self.get_style(super::groups::FOREGROUND)
            .unwrap_or_default()
    }
}

#[cfg(test)]
#[path = "theme_tests.rs"]
mod tests;
