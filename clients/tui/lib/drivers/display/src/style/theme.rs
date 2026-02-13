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
    fn get_style(&self, group: &str) -> Option<Style> {
        super::builtin::get_palette(self.variant)
            .get(group)
            .cloned()
    }

    fn name(&self) -> &str {
        self.variant.name()
    }

    fn default_style(&self) -> Style {
        // Return foreground style as default
        self.get_style(super::groups::FOREGROUND)
            .unwrap_or_default()
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

    // =========================================================================
    // Theme Completeness Tests (#439)
    // =========================================================================

    #[test]
    fn test_dark_theme_all_groups_defined() {
        let theme = BuiltinTheme::Dark.load();
        for group in super::super::groups::ALL_GROUPS {
            assert!(theme.get_style(group).is_some(), "Dark theme missing group: {group}");
        }
    }

    #[test]
    fn test_light_theme_all_groups_defined() {
        let theme = BuiltinTheme::Light.load();
        for group in super::super::groups::ALL_GROUPS {
            assert!(theme.get_style(group).is_some(), "Light theme missing group: {group}");
        }
    }

    #[test]
    fn test_tokyo_night_all_groups_defined() {
        let theme = BuiltinTheme::TokyoNightOrange.load();
        for group in super::super::groups::ALL_GROUPS {
            assert!(
                theme.get_style(group).is_some(),
                "TokyoNightOrange theme missing group: {group}"
            );
        }
    }

    #[test]
    fn test_theme_colors_are_different() {
        let dark = BuiltinTheme::Dark.load();
        let light = BuiltinTheme::Light.load();

        // Keywords should have different colors between themes
        let dark_keyword = dark.get_style(super::super::groups::KEYWORD).unwrap();
        let light_keyword = light.get_style(super::super::groups::KEYWORD).unwrap();
        assert_ne!(dark_keyword.fg, light_keyword.fg);
    }

    #[test]
    fn test_default_style_returns_foreground() {
        let theme = BuiltinTheme::Dark.load();
        let default = theme.default_style();
        let foreground = theme.get_style(super::super::groups::FOREGROUND).unwrap();

        // Default style should match foreground
        assert_eq!(default.fg, foreground.fg);
    }

    /// Test that the default trait impl of `default_style()` returns `Style::default()`.
    ///
    /// This tests the trait's provided default implementation, not the
    /// `SimpleBuiltinTheme` override.
    #[test]
    fn test_theme_provider_trait_default_style() {
        struct MinimalTheme;

        #[cfg_attr(coverage_nightly, coverage(off))]
        #[allow(clippy::unnecessary_literal_bound)]
        impl ThemeProvider for MinimalTheme {
            fn get_style(&self, _group: &str) -> Option<Style> {
                None
            }
            fn name(&self) -> &str {
                "minimal"
            }
            // Intentionally NOT overriding default_style() to test the trait default
        }

        let theme = MinimalTheme;
        let default = theme.default_style();
        assert_eq!(default, Style::default());
    }
}
