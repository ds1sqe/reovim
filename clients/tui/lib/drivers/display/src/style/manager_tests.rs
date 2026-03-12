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
