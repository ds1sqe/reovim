//! Tests for [`DisplayThemeFactory`] and [`install_theme_factory`].
//!
//! Note on test isolation: `install_theme_factory` writes to a
//! process-wide `OnceLock`. The display crate's other tests that go
//! through `BuiltinTheme::load()` rely on the factory being
//! installed. To make ordering deterministic, this module installs
//! the factory in a `#[ctor]`-style helper that runs lazily on first
//! access through [`ensure_factory_installed`]; every test in this
//! crate that touches `BuiltinTheme::load` / `ThemeLoader::load`
//! calls it first.

use std::sync::Once;

use reovim_driver_display_registry::theme::{BuiltinTheme, ThemeFactory as _, theme_factory};

use {
    super::{DisplayThemeFactory, install_theme_factory},
    crate::style::theme::StyledTheme,
};

static INSTALL: Once = Once::new();

/// Ensure the display-tier theme factory is registered before code
/// under test reaches `BuiltinTheme::load()` / `ThemeLoader::load`.
///
/// Idempotent across tests: only the first caller actually registers
/// (`OnceLock` semantics inside `set_theme_factory`).
pub fn ensure_factory_installed() {
    INSTALL.call_once(install_theme_factory);
}

#[test]
fn install_registers_factory() {
    ensure_factory_installed();
    assert!(theme_factory().is_some(), "factory must be installed");
}

#[test]
fn factory_load_builtin_returns_styled_dark() {
    ensure_factory_installed();
    let factory = DisplayThemeFactory;
    let provider = factory.load_builtin(BuiltinTheme::Dark);
    assert_eq!(provider.name(), "dark");

    let any = provider.as_any();
    let styled = any
        .downcast_ref::<super::super::theme::SimpleBuiltinTheme>()
        .expect("SimpleBuiltinTheme is the concrete type");
    assert!(styled.get_style("keyword").is_some());
}

#[test]
fn factory_load_builtin_light() {
    let factory = DisplayThemeFactory;
    let provider = factory.load_builtin(BuiltinTheme::Light);
    assert_eq!(provider.name(), "light");
}

#[test]
fn factory_load_builtin_tokyo_night() {
    let factory = DisplayThemeFactory;
    let provider = factory.load_builtin(BuiltinTheme::TokyoNightOrange);
    assert_eq!(provider.name(), "tokyo-night-orange");
}

#[test]
fn factory_load_file_parses_minimal_toml() {
    let factory = DisplayThemeFactory;
    let provider = factory
        .load_file("ignored", "[meta]\nname = \"From TOML\"\n")
        .unwrap();
    assert_eq!(provider.name(), "From TOML");
}

#[test]
fn factory_load_file_propagates_parse_error() {
    let factory = DisplayThemeFactory;
    let result = factory.load_file("ignored", "{{not toml");
    assert!(result.is_err());
}

#[test]
fn factory_load_file_propagates_color_error() {
    let factory = DisplayThemeFactory;
    let toml = "[meta]\nname = \"Bad\"\n[syntax]\nkeyword = { fg = \"not-a-color\" }\n";
    let result = factory.load_file("ignored", toml);
    assert!(result.is_err());
}

#[test]
fn factory_is_send_sync_clone_copy() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<DisplayThemeFactory>();

    let f = DisplayThemeFactory;
    let copy: DisplayThemeFactory = f;
    let clone = f;

    let dbg = format!("{f:?}");
    assert!(dbg.contains("DisplayThemeFactory"));
    assert_eq!(format!("{copy:?}"), format!("{clone:?}"));
}
