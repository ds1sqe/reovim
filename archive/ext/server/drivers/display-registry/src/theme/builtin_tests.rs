//! Tests for [`BuiltinTheme`].
//!
//! The `load()` method depends on the process-global theme factory
//! slot, which is mutated by `factory_tests::factory_singleton_set_once_wins`.
//! These tests therefore avoid asserting on the *concrete* provider
//! returned by `load()`; they only assert the parts that are
//! deterministic regardless of factory state (name, all, equality,
//! default).

use {
    super::{BuiltinTheme, StubBuiltinTheme},
    crate::theme::ThemeProvider,
};

#[test]
fn names_are_stable() {
    assert_eq!(BuiltinTheme::Dark.name(), "dark");
    assert_eq!(BuiltinTheme::Light.name(), "light");
    assert_eq!(BuiltinTheme::TokyoNightOrange.name(), "tokyo-night-orange");
}

#[test]
fn all_lists_three_variants() {
    let all = BuiltinTheme::all();
    assert_eq!(all.len(), 3);
    assert!(all.contains(&BuiltinTheme::Dark));
    assert!(all.contains(&BuiltinTheme::Light));
    assert!(all.contains(&BuiltinTheme::TokyoNightOrange));
}

#[test]
fn default_is_dark() {
    assert_eq!(BuiltinTheme::default(), BuiltinTheme::Dark);
}

#[test]
fn copy_clone_eq() {
    let dark = BuiltinTheme::Dark;
    let dark_copy = dark;
    assert_eq!(dark, dark_copy);
    assert_ne!(dark, BuiltinTheme::Light);
}

#[test]
fn debug_formats_variants() {
    assert!(format!("{:?}", BuiltinTheme::Dark).contains("Dark"));
    assert!(format!("{:?}", BuiltinTheme::Light).contains("Light"));
    assert!(format!("{:?}", BuiltinTheme::TokyoNightOrange).contains("TokyoNightOrange"));
}

#[test]
fn load_returns_some_provider() {
    // When no factory is registered, load() returns a stub whose name
    // matches the variant's name. If the factory test has already
    // installed a factory (test ordering in `cargo test`), the
    // factory's stub also reports a name. This test must be robust
    // against test-ordering, so it only asserts that load() does not
    // panic and returns *some* provider with a non-empty name.
    let provider = BuiltinTheme::Dark.load();
    assert!(!provider.name().is_empty());
}

#[test]
fn stub_builtin_theme_reports_variant_name() {
    // Direct stub test that does not depend on the factory global,
    // exercising the fallback construction path.
    for variant in BuiltinTheme::all() {
        let stub = StubBuiltinTheme { variant: *variant };
        assert_eq!(stub.name(), variant.name());
    }
}
