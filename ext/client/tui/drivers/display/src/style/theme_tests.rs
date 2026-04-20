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
