use super::*;

#[test]
fn test_builder_default() {
    let mut registry = DisplayRegistry::new();
    let id = ComponentId::new(1);

    registry
        .builder(id)
        .default(" TEST ", "T ", Style::default())
        .register();

    let info = registry.get(id).expect("should have registered");
    assert_eq!(info.display_string, " TEST ");
    assert_eq!(info.icon, "T ");
}

#[test]
fn test_builder_with_dynamic() {
    let mut registry = DisplayRegistry::new();
    let id = ComponentId::new(2);

    registry
        .builder(id)
        .default(" STATIC ", "S ", Style::default())
        .dynamic(|_ctx| Some("DYNAMIC".to_string()))
        .register();

    // Static info should be available
    let info = registry.get(id).expect("should have registered");
    assert_eq!(info.display_string, " STATIC ");

    // Static display_string returns the static string (no context needed)
    assert_eq!(registry.display_string(id), " STATIC ");

    // Dynamic should override when called with display_string_owned
    let display = registry.display_string_owned(id, Some(&()));
    assert_eq!(display, "DYNAMIC");
}

#[test]
#[should_panic(expected = "default() must be called")]
fn test_builder_panics_without_default() {
    let mut registry = DisplayRegistry::new();
    let id = ComponentId::new(3);

    registry.builder(id).register();
}
