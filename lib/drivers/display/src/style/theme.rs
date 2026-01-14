//! Theme provider trait for style lookups.
//!
//! Defines the interface for theme implementations and provides
//! built-in themes.

use std::sync::Arc;

use crate::highlight::Style;

/// Theme provides styles for highlight groups.
///
/// # Example
///
/// ```ignore
/// struct CustomTheme { /* ... */ }
///
/// impl ThemeProvider for CustomTheme {
///     fn get_style(&self, group: &str) -> Option<Style> {
///         match group {
///             "keyword" => Some(Style { fg: Some(Color::Blue), ..Default::default() }),
///             _ => None,
///         }
///     }
///
///     fn name(&self) -> &str { "custom" }
/// }
/// ```
pub trait ThemeProvider: Send + Sync {
    /// Get the style for a highlight group by name.
    ///
    /// Returns `None` if the group is not defined in this theme.
    fn get_style(&self, group: &str) -> Option<Style>;

    /// Get the theme name.
    fn name(&self) -> &str;

    /// Get the default/fallback style.
    fn default_style(&self) -> Style {
        Style::default()
    }
}

/// Built-in theme variants.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum BuiltinTheme {
    /// Dark theme (OneDark-inspired)
    #[default]
    Dark,
    /// Light theme
    Light,
    /// Tokyo Night Orange variant
    TokyoNightOrange,
}

impl BuiltinTheme {
    /// Load the theme as a `ThemeProvider`.
    ///
    /// This creates a boxed theme that can be used with `ThemeManager`.
    #[must_use]
    pub fn load(self) -> Arc<dyn ThemeProvider> {
        Arc::new(SimpleBuiltinTheme { variant: self })
    }

    /// Get the theme name.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Dark => "dark",
            Self::Light => "light",
            Self::TokyoNightOrange => "tokyo-night-orange",
        }
    }

    /// List all available built-in themes.
    #[must_use]
    pub const fn all() -> &'static [Self] {
        &[Self::Dark, Self::Light, Self::TokyoNightOrange]
    }
}

/// Simple implementation of built-in themes.
///
/// Theme definitions are self-contained in this crate.
struct SimpleBuiltinTheme {
    variant: BuiltinTheme,
}

impl ThemeProvider for SimpleBuiltinTheme {
    fn get_style(&self, _group: &str) -> Option<Style> {
        // TODO: Implement actual theme lookups with full style definitions
        // For now, return None to use default styles
        None
    }

    fn name(&self) -> &str {
        self.variant.name()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builtin_theme_names() {
        assert_eq!(BuiltinTheme::Dark.name(), "dark");
        assert_eq!(BuiltinTheme::Light.name(), "light");
        assert_eq!(BuiltinTheme::TokyoNightOrange.name(), "tokyo-night-orange");
    }

    #[test]
    fn test_builtin_theme_load() {
        let theme = BuiltinTheme::Dark.load();
        assert_eq!(theme.name(), "dark");
    }

    #[test]
    fn test_builtin_theme_all() {
        let all = BuiltinTheme::all();
        assert_eq!(all.len(), 3);
    }
}
