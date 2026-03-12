use reovim_arch::Color;

use super::*;

#[test]
fn test_style_group_registry_new() {
    let registry = StyleGroupRegistry::new();
    assert!(registry.is_empty());
}

#[test]
fn test_style_group_registry_register() {
    let registry = StyleGroupRegistry::new();
    let style = Style::new().fg(Color::Red);
    registry.register("test.group", style.clone());

    assert_eq!(registry.len(), 1);
    assert!(registry.contains("test.group"));
    assert_eq!(registry.get("test.group"), Some(style));
}

#[test]
fn test_style_group_registry_register_batch() {
    let registry = StyleGroupRegistry::new();
    let registrations = [
        ("group.1", Style::new().fg(Color::Red)),
        ("group.2", Style::new().fg(Color::Blue)),
    ];
    registry.register_batch(&registrations);

    assert_eq!(registry.len(), 2);
    assert!(registry.contains("group.1"));
    assert!(registry.contains("group.2"));
}

#[test]
fn test_style_group_registry_get_nonexistent() {
    let registry = StyleGroupRegistry::new();
    assert!(registry.get("nonexistent").is_none());
}

#[test]
fn test_style_group_registry_registered_groups() {
    let registry = StyleGroupRegistry::new();
    registry.register("a", Style::new());
    registry.register("b", Style::new());

    let groups = registry.registered_groups();
    assert_eq!(groups.len(), 2);
    assert!(groups.contains(&"a"));
    assert!(groups.contains(&"b"));
}

#[test]
fn test_style_group_registry_default() {
    let registry = StyleGroupRegistry::default();
    assert!(registry.is_empty());
    assert_eq!(registry.len(), 0);
    assert!(registry.get("anything").is_none());
}
