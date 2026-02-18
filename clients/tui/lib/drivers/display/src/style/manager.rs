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

    /// Get the style for a highlight group with hierarchical fallback.
    ///
    /// Uses 4-tier lookup order with hierarchical fallback:
    /// 1. User overrides (`set_override`)
    /// 2. Current theme (`ThemeProvider.get_style`)
    /// 3. Module defaults (`StyleGroupRegistry`)
    /// 4. Theme default (`ThemeProvider.default_style`)
    ///
    /// **Hierarchical Fallback**: If a group like `"keyword.control"` is not found,
    /// the lookup walks up the hierarchy: `"keyword.control"` → `"keyword"` → default.
    /// This allows themes to define broad styles and override specific variants.
    #[must_use]
    pub fn get_style(&self, group: &str) -> Style {
        // Try exact match with full 4-tier lookup
        if let Some(style) = self.lookup_exact(group) {
            return style;
        }

        // Hierarchical fallback: walk up the dot-separated hierarchy
        let mut current = group;
        while let Some((parent, _)) = current.rsplit_once('.') {
            if let Some(style) = self.lookup_exact(parent) {
                return style;
            }
            current = parent;
        }

        // Final fallback to theme default
        self.current.default_style()
    }

    /// Perform exact lookup through all 4 tiers (no hierarchical fallback).
    ///
    /// This is used internally by `get_style` for each level of the hierarchy.
    fn lookup_exact(&self, group: &str) -> Option<Style> {
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

    // =========================================================================
    // Hierarchical Fallback Tests (Phase 13.0 #470)
    // =========================================================================

    #[test]
    fn test_hierarchical_fallback_single_level() {
        use super::StyleGroupRegistry;

        let mut manager = ThemeManager::new(BuiltinTheme::Dark.load());

        // Register a style for "custom" but not "custom.specific"
        let registry = Arc::new(StyleGroupRegistry::new());
        registry.register("custom", Style::new().fg(Color::Yellow));
        manager.set_module_defaults(registry);

        // "custom.specific" should fall back to "custom"
        let style = manager.get_style("custom.specific");
        assert_eq!(style.fg, Some(Color::Yellow));
    }

    #[test]
    fn test_hierarchical_fallback_multi_level() {
        use super::StyleGroupRegistry;

        let mut manager = ThemeManager::new(BuiltinTheme::Dark.load());

        // Register only the base level
        let registry = Arc::new(StyleGroupRegistry::new());
        registry.register("my", Style::new().fg(Color::Cyan));
        manager.set_module_defaults(registry);

        // "my.deep.nested.group" should fall back through the hierarchy
        // my.deep.nested.group → my.deep.nested → my.deep → my
        let style = manager.get_style("my.deep.nested.group");
        assert_eq!(style.fg, Some(Color::Cyan));
    }

    #[test]
    fn test_hierarchical_fallback_prefers_specific() {
        use super::StyleGroupRegistry;

        let mut manager = ThemeManager::new(BuiltinTheme::Dark.load());

        // Register both base and specific
        let registry = Arc::new(StyleGroupRegistry::new());
        registry.register("test", Style::new().fg(Color::Red));
        registry.register("test.specific", Style::new().fg(Color::Blue));
        manager.set_module_defaults(registry);

        // Specific should be returned (not fallback to base)
        let specific = manager.get_style("test.specific");
        assert_eq!(specific.fg, Some(Color::Blue));

        // Base should return base
        let base = manager.get_style("test");
        assert_eq!(base.fg, Some(Color::Red));
    }

    #[test]
    fn test_hierarchical_fallback_to_default() {
        let manager = ThemeManager::new(BuiltinTheme::Dark.load());

        // "completely.unknown.group" should fall back to default style
        let style = manager.get_style("completely.unknown.group");
        assert_eq!(style, manager.current_theme().default_style());
    }

    // =========================================================================
    // Coverage tests for manager.rs uncovered paths
    // =========================================================================

    #[test]
    fn test_module_defaults_accessor() {
        let mut manager = ThemeManager::new(BuiltinTheme::Dark.load());

        // Initially None
        assert!(manager.module_defaults().is_none());

        // After setting, should be Some
        let registry = Arc::new(StyleGroupRegistry::new());
        manager.set_module_defaults(registry);
        assert!(manager.module_defaults().is_some());
    }

    #[test]
    fn test_try_get_style_returns_override() {
        let mut manager = ThemeManager::new(BuiltinTheme::Dark.load());
        let override_style = Style::new().fg(Color::Magenta);
        manager.set_override("custom.test", override_style);

        // try_get_style should find the override (tier 1)
        let style = manager.try_get_style("custom.test");
        assert!(style.is_some());
        assert_eq!(style.unwrap().fg, Some(Color::Magenta));
    }

    #[test]
    fn test_shared_theme_manager_with_module_defaults() {
        let registry = Arc::new(StyleGroupRegistry::new());
        registry.register("shared.test", Style::new().fg(Color::Yellow));

        let shared = SharedThemeManager::with_module_defaults(BuiltinTheme::Dark.load(), registry);

        // Verify theme name through read lock
        assert_eq!(shared.read().current_theme_name(), "dark");

        // Verify module defaults are set
        assert!(shared.read().module_defaults().is_some());

        // Verify the registered style is accessible
        let style = shared.read().get_style("shared.test");
        assert_eq!(style.fg, Some(Color::Yellow));
    }

    #[test]
    fn test_try_get_style_no_module_defaults() {
        // module_defaults is None in try_get_style (line 202 else branch)
        let manager = ThemeManager::new(BuiltinTheme::Dark.load());
        // No module_defaults set, query a group not in overrides or theme
        let style = manager.try_get_style("nonexistent.group");
        assert!(style.is_none());
    }

    #[test]
    fn test_shared_theme_manager_write_set_theme() {
        let shared = SharedThemeManager::new(BuiltinTheme::Dark.load());
        assert_eq!(shared.read().current_theme_name(), "dark");

        shared.write().set_theme(BuiltinTheme::Light.load());
        assert_eq!(shared.read().current_theme_name(), "light");
    }
}
