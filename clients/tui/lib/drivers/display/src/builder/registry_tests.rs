use crate::highlight::Style;

use super::*;

#[test]
fn test_registry_new() {
    let registry = DisplayRegistry::new();
    assert!(registry.is_empty());
    assert_eq!(registry.len(), 0);
}

#[test]
fn test_registry_get() {
    let mut registry = DisplayRegistry::new();
    let id = ComponentId::new(1);

    registry
        .builder(id)
        .default(" TEST ", "T ", Style::default())
        .register();

    assert!(registry.contains(id));
    let info = registry.get(id).unwrap();
    assert_eq!(info.display_string, " TEST ");
}

#[test]
fn test_registry_icon_fallback() {
    let registry = DisplayRegistry::new();
    let id = ComponentId::new(999);

    // Unregistered component should return fallback
    assert_eq!(registry.icon(id), fallback::NONE);
}

#[test]
fn test_registry_display_string_owned_dynamic() {
    let mut registry = DisplayRegistry::new();
    let id = ComponentId::new(1);

    registry
        .builder(id)
        .default(" STATIC ", "S ", Style::default())
        .dynamic(|ctx| ctx.downcast_ref::<i32>().map(|n| format!(" COUNT: {n} ")))
        .register();

    // Without context, should return static
    let display = registry.display_string_owned(id, None);
    assert_eq!(display, " STATIC ");

    // With matching context, should return dynamic
    let count: i32 = 42;
    let display = registry.display_string_owned(id, Some(&count));
    assert_eq!(display, " COUNT: 42 ");

    // With non-matching context, should return static
    let text = "hello";
    let display = registry.display_string_owned(id, Some(&text));
    assert_eq!(display, " STATIC ");
}

#[test]
fn test_registry_remove() {
    let mut registry = DisplayRegistry::new();
    let id = ComponentId::new(1);

    registry
        .builder(id)
        .default(" TEST ", "T ", Style::default())
        .register();

    assert!(registry.contains(id));
    let removed = registry.remove(id);
    assert!(removed.is_some());
    assert!(!registry.contains(id));
}

#[test]
fn test_registry_display_string_owned_unregistered() {
    let registry = DisplayRegistry::new();
    let id = ComponentId::new(999);

    // Unregistered component should return empty string
    let display = registry.display_string_owned(id, None);
    assert_eq!(display, "");
}

#[test]
fn test_registry_clear() {
    let mut registry = DisplayRegistry::new();

    registry
        .builder(ComponentId::new(1))
        .default(" A ", "A ", Style::default())
        .register();
    registry
        .builder(ComponentId::new(2))
        .default(" B ", "B ", Style::default())
        .register();

    assert_eq!(registry.len(), 2);
    registry.clear();
    assert!(registry.is_empty());
}
