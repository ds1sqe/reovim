//! Tests for the display-tier [`StyledThemeManagerExt`] extension.
//!
//! The slim [`ThemeManager`] (re-exported from
//! `reovim-driver-display-registry`) is exercised in the registry
//! crate's own test suite. The tests here focus on the
//! `Style`-aware extension: hierarchical fallback, theme default
//! fallback, and the `Any`-based downcast that recovers the styled
//! view.

use std::sync::{Arc, Once};

use {
    reovim_arch::Color,
    reovim_driver_display_registry::theme::{ThemeManager, ThemeProvider},
};

use {
    super::StyledThemeManagerExt,
    crate::{
        highlight::Style,
        style::{BuiltinTheme, install_theme_factory},
    },
};

static INSTALL: Once = Once::new();

fn ensure_factory_installed() {
    INSTALL.call_once(install_theme_factory);
}

#[test]
fn manager_get_style_resolves_known_group() {
    ensure_factory_installed();
    let manager = ThemeManager::new(BuiltinTheme::Dark.load());
    let keyword = manager.get_style("keyword");
    assert!(keyword.fg.is_some());
}

#[test]
fn manager_get_style_falls_back_to_theme_default() {
    ensure_factory_installed();
    let manager = ThemeManager::new(BuiltinTheme::Dark.load());
    let unknown = manager.get_style("completely.unknown.group");
    let styled = manager
        .current_styled_theme()
        .expect("dark theme is StyledTheme");
    assert_eq!(unknown, styled.default_style());
}

#[test]
fn manager_get_style_walks_hierarchy_single_level() {
    ensure_factory_installed();
    let manager = ThemeManager::new(BuiltinTheme::Dark.load());
    // keyword.control inherits keyword in the dark palette.
    let style = manager.get_style("keyword.control");
    let parent = manager.get_style("keyword");
    assert_eq!(style.fg, parent.fg);
}

#[test]
fn manager_get_style_walks_hierarchy_multiple_levels() {
    ensure_factory_installed();
    let manager = ThemeManager::new(BuiltinTheme::Dark.load());
    // keyword.control.flow.special falls all the way back to keyword.
    let style = manager.get_style("keyword.control.flow.special");
    let parent = manager.get_style("keyword");
    assert_eq!(style.fg, parent.fg);
}

#[test]
fn manager_set_theme_changes_lookups() {
    ensure_factory_installed();
    let mut manager = ThemeManager::new(BuiltinTheme::Dark.load());
    let dark_keyword = manager.get_style("keyword");

    manager.set_theme(BuiltinTheme::Light.load());
    let light_keyword = manager.get_style("keyword");

    assert_ne!(dark_keyword.fg, light_keyword.fg);
}

#[test]
fn manager_get_style_for_unstyled_provider_returns_default() {
    // A theme provider that does NOT implement StyledTheme: the
    // extension's `current_styled_theme()` returns None and
    // `get_style()` falls back to `Style::default()`.
    struct OpaqueProvider;
    impl ThemeProvider for OpaqueProvider {
        fn name(&self) -> &'static str {
            "opaque"
        }

        fn as_any(&self) -> &dyn std::any::Any {
            self
        }
    }
    let manager = ThemeManager::new(Arc::new(OpaqueProvider));
    assert!(manager.current_styled_theme().is_none());
    assert_eq!(manager.get_style("keyword"), Style::default());
}

#[test]
fn manager_styled_theme_recovers_simple_builtin() {
    ensure_factory_installed();
    let manager = ThemeManager::new(BuiltinTheme::Dark.load());
    assert!(manager.current_styled_theme().is_some());
}

#[test]
fn manager_styled_theme_recovers_file_theme() {
    use crate::style::FileTheme;
    let theme =
        FileTheme::parse("[meta]\nname = \"Custom\"\n[syntax]\nkeyword = { fg = \"#ff0000\" }\n")
            .unwrap();
    let manager = ThemeManager::new(theme.into_arc());
    assert!(manager.current_styled_theme().is_some());
    let style = manager.get_style("keyword");
    assert_eq!(style.fg, Some(Color::Rgb { r: 255, g: 0, b: 0 }));
}

#[test]
fn extension_compiles_with_color_value() {
    // Smoke test that the extension trait is in scope and the
    // returned Style carries an RGB Color value from the palette.
    ensure_factory_installed();
    let manager = ThemeManager::new(BuiltinTheme::Dark.load());
    let style = manager.get_style("git.add");
    let color = style.fg.expect("git.add has a foreground color");
    assert!(matches!(color, Color::Rgb { .. }));
}
