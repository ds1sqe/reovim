use std::{any::Any, sync::Arc};

use {
    super::{SharedThemeManager, ThemeManager},
    crate::theme::ThemeProvider,
};

struct NamedTheme {
    name: &'static str,
}

impl ThemeProvider for NamedTheme {
    fn name(&self) -> &str {
        self.name
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

#[test]
fn manager_reports_current_theme_name() {
    let manager = ThemeManager::new(Arc::new(NamedTheme { name: "alpha" }));
    assert_eq!(manager.current_theme_name(), "alpha");
}

#[test]
fn manager_set_theme_replaces_current() {
    let mut manager = ThemeManager::new(Arc::new(NamedTheme { name: "alpha" }));
    manager.set_theme(Arc::new(NamedTheme { name: "beta" }));
    assert_eq!(manager.current_theme_name(), "beta");
}

#[test]
fn manager_current_theme_borrow_matches_name() {
    let manager = ThemeManager::new(Arc::new(NamedTheme { name: "alpha" }));
    assert_eq!(manager.current_theme().name(), "alpha");
}

#[test]
fn shared_manager_read_write_round_trip() {
    let shared = SharedThemeManager::new(Arc::new(NamedTheme { name: "alpha" }));
    assert_eq!(shared.read().current_theme_name(), "alpha");

    shared
        .write()
        .set_theme(Arc::new(NamedTheme { name: "beta" }));
    assert_eq!(shared.read().current_theme_name(), "beta");
}

#[test]
fn shared_manager_is_send_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<SharedThemeManager>();
    assert_send_sync::<ThemeManager>();
}
