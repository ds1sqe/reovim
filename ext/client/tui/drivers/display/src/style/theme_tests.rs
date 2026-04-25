//! Tests for the display-tier `StyledTheme` super-trait and the
//! `SimpleBuiltinTheme` concrete impl.

use std::sync::Once;

use {
    super::SimpleBuiltinTheme,
    crate::{
        highlight::Style,
        style::{BuiltinTheme, StyledTheme, install_theme_factory},
    },
};

static INSTALL: Once = Once::new();

/// Local `ensure_factory_installed` mirror; idempotent across this
/// crate's test binary thanks to `Once` and `OnceLock` semantics in
/// `set_theme_factory`.
fn ensure_factory_installed() {
    INSTALL.call_once(install_theme_factory);
}

#[test]
fn builtin_theme_names_are_stable() {
    assert_eq!(BuiltinTheme::Dark.name(), "dark");
    assert_eq!(BuiltinTheme::Light.name(), "light");
    assert_eq!(BuiltinTheme::TokyoNightOrange.name(), "tokyo-night-orange");
}

#[test]
fn builtin_theme_load_returns_named_provider() {
    ensure_factory_installed();
    let theme = BuiltinTheme::Dark.load();
    assert_eq!(theme.name(), "dark");
}

#[test]
fn builtin_theme_all_lists_three() {
    let all = BuiltinTheme::all();
    assert_eq!(all.len(), 3);
}

#[test]
fn dark_theme_defines_all_groups() {
    let theme = SimpleBuiltinTheme {
        variant: BuiltinTheme::Dark,
    };
    for group in super::super::groups::ALL_GROUPS {
        assert!(theme.get_style(group).is_some(), "Dark missing group: {group}");
    }
}

#[test]
fn light_theme_defines_all_groups() {
    let theme = SimpleBuiltinTheme {
        variant: BuiltinTheme::Light,
    };
    for group in super::super::groups::ALL_GROUPS {
        assert!(theme.get_style(group).is_some(), "Light missing group: {group}");
    }
}

#[test]
fn tokyo_night_defines_all_groups() {
    let theme = SimpleBuiltinTheme {
        variant: BuiltinTheme::TokyoNightOrange,
    };
    for group in super::super::groups::ALL_GROUPS {
        assert!(theme.get_style(group).is_some(), "TokyoNightOrange missing group: {group}");
    }
}

#[test]
fn theme_colors_differ_between_variants() {
    let dark = SimpleBuiltinTheme {
        variant: BuiltinTheme::Dark,
    };
    let light = SimpleBuiltinTheme {
        variant: BuiltinTheme::Light,
    };
    let dark_keyword = dark.get_style(super::super::groups::KEYWORD).unwrap();
    let light_keyword = light.get_style(super::super::groups::KEYWORD).unwrap();
    assert_ne!(dark_keyword.fg, light_keyword.fg);
}

#[test]
fn default_style_returns_foreground() {
    let theme = SimpleBuiltinTheme {
        variant: BuiltinTheme::Dark,
    };
    let default = theme.default_style();
    let foreground = theme.get_style(super::super::groups::FOREGROUND).unwrap();
    assert_eq!(default.fg, foreground.fg);
}

/// `StyledTheme::default_style` has a provided default that returns
/// `Style::default()`. This test exercises the trait-default path
/// (not the `SimpleBuiltinTheme` override).
#[test]
fn styled_theme_trait_default_style() {
    use reovim_driver_display_registry::theme::ThemeProvider;

    struct Minimal;

    impl ThemeProvider for Minimal {
        fn name(&self) -> &'static str {
            "minimal"
        }

        fn as_any(&self) -> &dyn std::any::Any {
            self
        }
    }

    impl StyledTheme for Minimal {
        fn get_style(&self, _group: &str) -> Option<Style> {
            None
        }
        // intentionally NOT overriding default_style()
    }

    let theme = Minimal;
    assert_eq!(theme.default_style(), Style::default());
}
