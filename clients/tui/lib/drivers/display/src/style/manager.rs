//! Theme manager for runtime theme management.
//!
//! Provides centralized theme management with support for overrides.
//!
//! # Lookup Order
//!
//! When getting a style for a group, the 4-tier lookup is:
//!
//! 1. **User overrides** (`set_override`)
//! 2. **Current theme** (`ThemeProvider.get_style`)
//! 3. **Module defaults** (`StyleGroupRegistry`) - modules register their defaults
//! 4. **Theme default** (`ThemeProvider.default_style`)

use std::{collections::HashMap, sync::Arc};

use crate::highlight::Style;

use super::{registry::StyleGroupRegistry, theme::ThemeProvider};

/// Manages the current theme and user overrides.
///
/// # Ownership
///
/// The runner creates and owns the `ThemeManager`. This is a policy decision
/// (which theme to use), not mechanism.
///
/// # 4-Tier Lookup
///
/// ```text
/// 1. User overrides (set_override)
/// 2. Current theme (ThemeProvider.get_style)
/// 3. Module defaults (StyleGroupRegistry)
/// 4. ThemeProvider.default_style()
/// ```
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
/// // Get style (checks 4 tiers: overrides → theme → module defaults → fallback)
/// let style = manager.get_style("keyword");
/// ```
pub struct ThemeManager {
    /// Current theme
    current: Arc<dyn ThemeProvider>,
    /// User style overrides (take precedence over theme)
    overrides: HashMap<String, Style>,
    /// Module-provided style defaults (fallback when theme doesn't define a group)
    module_defaults: Option<Arc<StyleGroupRegistry>>,
}

impl ThemeManager {
    /// Create a new theme manager with the given theme.
    #[must_use]
    pub fn new(theme: Arc<dyn ThemeProvider>) -> Self {
        Self {
            current: theme,
            overrides: HashMap::new(),
            module_defaults: None,
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

    /// Set the module defaults registry.
    ///
    /// Modules register their style group defaults here during `init()`.
    /// The registry is used as tier 3 in the lookup order.
    pub fn set_module_defaults(&mut self, registry: Arc<StyleGroupRegistry>) {
        self.module_defaults = Some(registry);
    }

    /// Get the module defaults registry.
    #[must_use]
    pub const fn module_defaults(&self) -> Option<&Arc<StyleGroupRegistry>> {
        self.module_defaults.as_ref()
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
    /// Uses 4-tier lookup order:
    /// 1. User overrides (`set_override`)
    /// 2. Current theme (`ThemeProvider.get_style`)
    /// 3. Module defaults (`StyleGroupRegistry`)
    /// 4. Theme default (`ThemeProvider.default_style`)
    #[must_use]
    pub fn get_style(&self, group: &str) -> Style {
        // Tier 1: User overrides
        if let Some(style) = self.overrides.get(group) {
            return style.clone();
        }

        // Tier 2: Current theme
        if let Some(style) = self.current.get_style(group) {
            return style;
        }

        // Tier 3: Module defaults
        if let Some(ref registry) = self.module_defaults
            && let Some(style) = registry.get(group)
        {
            return style;
        }

        // Tier 4: Theme default
        self.current.default_style()
    }

    /// Get the style for a highlight group, returning None if not found.
    ///
    /// Checks tiers 1-3 only (overrides → theme → module defaults).
    /// Unlike `get_style`, this doesn't fall back to the theme's default style.
    #[must_use]
    pub fn try_get_style(&self, group: &str) -> Option<Style> {
        // Tier 1: User overrides
        if let Some(style) = self.overrides.get(group) {
            return Some(style.clone());
        }

        // Tier 2: Current theme
        if let Some(style) = self.current.get_style(group) {
            return Some(style);
        }

        // Tier 3: Module defaults
        if let Some(ref registry) = self.module_defaults
            && let Some(style) = registry.get(group)
        {
            return Some(style);
        }

        None
    }
}

// Implement Service marker trait for ServiceRegistry integration (#439)
impl reovim_kernel::api::v1::Service for ThemeManager {}

// ============================================================================
// SharedThemeManager - RwLock wrapper for ServiceRegistry (#439)
// ============================================================================

use reovim_arch::sync::RwLock;

/// Thread-safe wrapper for `ThemeManager` for `ServiceRegistry`.
///
/// This newtype wrapper allows `ThemeManager` to be stored in `ServiceRegistry`
/// while enabling mutation via `RwLock`. Use `read()` and `write()` to access
/// the inner manager.
///
/// # Example
///
/// ```ignore
/// use reovim_driver_display::style::SharedThemeManager;
///
/// // Register in ServiceRegistry
/// let manager = SharedThemeManager::new(BuiltinTheme::Dark.load());
/// services.register(Arc::new(manager));
///
/// // Access later
/// let shared = services.get::<SharedThemeManager>().unwrap();
/// let name = shared.read().current_theme_name();
/// shared.write().set_theme(BuiltinTheme::Light.load());
/// ```
pub struct SharedThemeManager(RwLock<ThemeManager>);

impl SharedThemeManager {
    /// Create a new shared theme manager.
    #[must_use]
    pub fn new(theme: Arc<dyn super::theme::ThemeProvider>) -> Self {
        Self(RwLock::new(ThemeManager::new(theme)))
    }

    /// Create a new shared theme manager with module defaults registry.
    #[must_use]
    pub fn with_module_defaults(
        theme: Arc<dyn super::theme::ThemeProvider>,
        registry: Arc<StyleGroupRegistry>,
    ) -> Self {
        let mut manager = ThemeManager::new(theme);
        manager.set_module_defaults(registry);
        Self(RwLock::new(manager))
    }

    /// Acquire a read lock on the theme manager.
    pub fn read(&self) -> reovim_arch::sync::RwLockReadGuard<'_, ThemeManager> {
        self.0.read()
    }

    /// Acquire a write lock on the theme manager.
    pub fn write(&self) -> reovim_arch::sync::RwLockWriteGuard<'_, ThemeManager> {
        self.0.write()
    }
}

impl reovim_kernel::api::v1::Service for SharedThemeManager {}

#[cfg(test)]
mod tests {
    use reovim_arch::Color;

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

    #[test]
    fn test_theme_manager_four_tier_lookup() {
        use super::StyleGroupRegistry;

        let mut manager = ThemeManager::new(BuiltinTheme::Dark.load());

        // Create module defaults registry
        let registry = Arc::new(StyleGroupRegistry::new());
        let module_style = Style::new().fg(Color::Cyan);
        registry.register("module.custom", module_style);
        manager.set_module_defaults(registry);

        // Tier 3: Module defaults - not in theme, not in overrides
        let style = manager.get_style("module.custom");
        assert_eq!(style.fg, Some(Color::Cyan));

        // Tier 2: Theme takes precedence over module defaults
        // keyword is in theme, so it should come from theme
        let theme_style = manager.get_style("keyword");
        assert!(theme_style.fg.is_some());
        assert_ne!(theme_style.fg, Some(Color::Cyan)); // Different from module style

        // Tier 1: Override takes precedence over everything
        let override_style = Style::new().fg(Color::Magenta);
        manager.set_override("module.custom", override_style);
        let style = manager.get_style("module.custom");
        assert_eq!(style.fg, Some(Color::Magenta));

        // Tier 4: Fallback to theme default for unknown groups
        let unknown_style = manager.get_style("nonexistent.group");
        assert_eq!(unknown_style, manager.current_theme().default_style());
    }

    #[test]
    fn test_theme_manager_try_get_style_tiers() {
        use super::StyleGroupRegistry;

        let mut manager = ThemeManager::new(BuiltinTheme::Dark.load());

        // Create module defaults registry
        let registry = Arc::new(StyleGroupRegistry::new());
        registry.register("module.test", Style::new().fg(Color::Green));
        manager.set_module_defaults(registry);

        // Should find in module defaults (tier 3)
        assert!(manager.try_get_style("module.test").is_some());

        // Should find in theme (tier 2)
        assert!(manager.try_get_style("keyword").is_some());

        // Should not find unknown (no tier 4 fallback in try_get_style)
        assert!(manager.try_get_style("nonexistent").is_none());
    }
}
