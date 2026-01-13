//! Theme provider trait for style lookups.
//!
//! Defines the interface for theme implementations and provides
//! built-in themes and adapters.

use std::sync::Arc;

use reovim_core::highlight::Style;

/// Theme provides styles for highlight groups.
///
/// Named `ThemeProvider` to avoid conflict with the existing
/// `reovim_core::highlight::Theme` struct.
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
/// For now, this delegates to the existing Theme system in reovim-core.
/// In future phases, theme definitions will move here.
struct SimpleBuiltinTheme {
    variant: BuiltinTheme,
}

impl ThemeProvider for SimpleBuiltinTheme {
    fn get_style(&self, _group: &str) -> Option<Style> {
        // TODO: Implement actual theme lookups
        // For now, return None to use default styles
        None
    }

    fn name(&self) -> &str {
        self.variant.name()
    }
}

/// Adapter to wrap the existing `reovim_core::highlight::Theme` as a `ThemeProvider`.
///
/// This provides compatibility with the existing theme system during migration.
pub struct CoreThemeAdapter {
    inner: reovim_core::highlight::Theme,
    name: String,
}

impl CoreThemeAdapter {
    /// Create a new adapter wrapping a core theme.
    #[must_use]
    pub fn new(theme: reovim_core::highlight::Theme, name: impl Into<String>) -> Self {
        Self {
            inner: theme,
            name: name.into(),
        }
    }

    /// Get a reference to the inner theme.
    #[must_use]
    pub const fn inner(&self) -> &reovim_core::highlight::Theme {
        &self.inner
    }
}

impl ThemeProvider for CoreThemeAdapter {
    fn get_style(&self, group: &str) -> Option<Style> {
        // Map common group names to theme fields
        // This is a simplified mapping; full implementation would be more comprehensive
        match group {
            "normal" | "default" => Some(self.inner.base.default.clone()),
            "cursor_line" => Some(self.inner.base.cursor_line.clone()),
            "command_line" => Some(self.inner.base.command_line.clone()),
            "line_number" => Some(self.inner.gutter.line_number.clone()),
            "current_line_number" => Some(self.inner.gutter.current_line_number.clone()),
            "visual" => Some(self.inner.selection.visual.clone()),
            "search" | "search_match" => Some(self.inner.search.match_highlight.clone()),
            "inc_search" => Some(self.inner.search.inc_search.clone()),
            "diagnostic_error" => Some(self.inner.diagnostic.error.clone()),
            "diagnostic_warn" | "diagnostic_warning" => Some(self.inner.diagnostic.warn.clone()),
            "diagnostic_info" => Some(self.inner.diagnostic.info.clone()),
            "diagnostic_hint" => Some(self.inner.diagnostic.hint.clone()),
            "statusline_normal" => Some(self.inner.statusline.mode.normal.clone()),
            "statusline_insert" => Some(self.inner.statusline.mode.insert.clone()),
            "statusline_visual" => Some(self.inner.statusline.mode.visual.clone()),
            "statusline_command" => Some(self.inner.statusline.mode.command.clone()),
            "popup" | "popup_normal" => Some(self.inner.popup.normal.clone()),
            "popup_selected" => Some(self.inner.popup.selected.clone()),
            "popup_match" => Some(self.inner.popup.match_fg.clone()),
            "tab_active" => Some(self.inner.tab.active.clone()),
            "tab_inactive" => Some(self.inner.tab.inactive.clone()),
            "window_separator" => Some(self.inner.window.separator.clone()),
            _ => None,
        }
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn default_style(&self) -> Style {
        self.inner.base.default.clone()
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

    #[test]
    fn test_core_theme_adapter() {
        let core_theme = reovim_core::highlight::Theme::default();
        let adapter = CoreThemeAdapter::new(core_theme, "test-theme");

        assert_eq!(adapter.name(), "test-theme");
        assert!(adapter.get_style("normal").is_some());
        assert!(adapter.get_style("nonexistent_group").is_none());
    }
}
