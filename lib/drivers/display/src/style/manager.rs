//! Theme manager for runtime theme management.
//!
//! Provides centralized theme management with support for overrides.

use std::{collections::HashMap, sync::Arc};

use reovim_core::highlight::Style;

use super::theme::ThemeProvider;

/// Manages the current theme and user overrides.
///
/// # Ownership
///
/// The runner creates and owns the `ThemeManager`. This is a policy decision
/// (which theme to use), not mechanism.
///
/// # Example
///
/// ```ignore
/// use reovim_driver_display::style::{ThemeManager, BuiltinTheme};
///
/// // Create with default dark theme
/// let mut manager = ThemeManager::new(BuiltinTheme::Dark.load());
///
/// // Override a specific style
/// manager.set_override("keyword", keyword_style);
///
/// // Get style (checks overrides first, then theme)
/// let style = manager.get_style("keyword");
/// ```
pub struct ThemeManager {
    /// Current theme
    current: Arc<dyn ThemeProvider>,
    /// User style overrides (take precedence over theme)
    overrides: HashMap<String, Style>,
}

impl ThemeManager {
    /// Create a new theme manager with the given theme.
    #[must_use]
    pub fn new(theme: Arc<dyn ThemeProvider>) -> Self {
        Self {
            current: theme,
            overrides: HashMap::new(),
        }
    }

    /// Set the current theme.
    pub fn set_theme(&mut self, theme: Arc<dyn ThemeProvider>) {
        self.current = theme;
    }

    /// Get the current theme.
    #[must_use]
    pub fn current_theme(&self) -> &Arc<dyn ThemeProvider> {
        &self.current
    }

    /// Get the current theme name.
    #[must_use]
    pub fn current_theme_name(&self) -> &str {
        self.current.name()
    }

    /// Set a style override for a highlight group.
    ///
    /// Overrides take precedence over theme-defined styles.
    pub fn set_override(&mut self, group: impl Into<String>, style: Style) {
        self.overrides.insert(group.into(), style);
    }

    /// Remove a style override.
    pub fn remove_override(&mut self, group: &str) -> Option<Style> {
        self.overrides.remove(group)
    }

    /// Clear all style overrides.
    pub fn clear_overrides(&mut self) {
        self.overrides.clear();
    }

    /// Check if a group has an override.
    #[must_use]
    pub fn has_override(&self, group: &str) -> bool {
        self.overrides.contains_key(group)
    }

    /// Get the number of overrides.
    #[must_use]
    pub fn override_count(&self) -> usize {
        self.overrides.len()
    }

    /// Get the style for a highlight group.
    ///
    /// Checks overrides first, then falls back to the theme.
    /// If neither has the group, returns the theme's default style.
    #[must_use]
    pub fn get_style(&self, group: &str) -> Style {
        // Check overrides first
        if let Some(style) = self.overrides.get(group) {
            return style.clone();
        }

        // Fall back to theme
        self.current
            .get_style(group)
            .unwrap_or_else(|| self.current.default_style())
    }

    /// Get the style for a highlight group, returning None if not found.
    ///
    /// Unlike `get_style`, this doesn't fall back to a default.
    #[must_use]
    pub fn try_get_style(&self, group: &str) -> Option<Style> {
        self.overrides
            .get(group)
            .cloned()
            .or_else(|| self.current.get_style(group))
    }
}

#[cfg(test)]
mod tests {
    use reovim_core::highlight::Color;

    use super::{super::theme::BuiltinTheme, *};

    #[test]
    fn test_theme_manager_creation() {
        let manager = ThemeManager::new(BuiltinTheme::Dark.load());
        assert_eq!(manager.current_theme_name(), "dark");
    }

    #[test]
    fn test_theme_manager_override() {
        let mut manager = ThemeManager::new(BuiltinTheme::Dark.load());

        let custom_style = Style {
            fg: Some(Color::Red),
            ..Default::default()
        };

        manager.set_override("custom_group", custom_style);

        assert!(manager.has_override("custom_group"));
        assert_eq!(manager.override_count(), 1);

        let retrieved = manager.get_style("custom_group");
        assert_eq!(retrieved.fg, Some(Color::Red));
    }

    #[test]
    fn test_theme_manager_remove_override() {
        let mut manager = ThemeManager::new(BuiltinTheme::Dark.load());

        let custom_style = Style {
            fg: Some(Color::Blue),
            ..Default::default()
        };

        manager.set_override("test", custom_style);
        assert!(manager.has_override("test"));

        manager.remove_override("test");
        assert!(!manager.has_override("test"));
    }

    #[test]
    fn test_theme_manager_clear_overrides() {
        let mut manager = ThemeManager::new(BuiltinTheme::Dark.load());

        manager.set_override("a", Style::default());
        manager.set_override("b", Style::default());
        manager.set_override("c", Style::default());

        assert_eq!(manager.override_count(), 3);

        manager.clear_overrides();
        assert_eq!(manager.override_count(), 0);
    }

    #[test]
    fn test_theme_manager_set_theme() {
        let mut manager = ThemeManager::new(BuiltinTheme::Dark.load());
        assert_eq!(manager.current_theme_name(), "dark");

        manager.set_theme(BuiltinTheme::Light.load());
        assert_eq!(manager.current_theme_name(), "light");
    }
}
