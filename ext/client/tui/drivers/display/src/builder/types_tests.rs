use super::*;

#[test]
fn test_component_id_creation() {
    let id = ComponentId::new(42);
    assert_eq!(id.as_u64(), 42);
}

#[test]
fn test_component_id_from_u64() {
    let id: ComponentId = 123_u64.into();
    assert_eq!(id.as_u64(), 123);
}

#[test]
fn test_component_id_equality() {
    let id1 = ComponentId::new(42);
    let id2 = ComponentId::new(42);
    let id3 = ComponentId::new(43);

    assert_eq!(id1, id2);
    assert_ne!(id1, id3);
}

#[test]
fn test_display_info_default() {
    let info = DisplayInfo::default();
    assert_eq!(info.display_string, "");
    assert_eq!(info.icon, "");
}
