use super::*;

#[test]
fn test_module_id() {
    let id = ModuleId::new("lang-rust");
    assert_eq!(id.as_str(), "lang-rust");
    assert_eq!(format!("{id}"), "lang-rust");
}

#[test]
fn test_module_id_equality() {
    let id1 = ModuleId::new("lang-rust");
    let id2 = ModuleId::new("lang-rust");
    let id3 = ModuleId::new("lang-python");
    assert_eq!(id1, id2);
    assert_ne!(id1, id3);
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_module_id_dynamic() {
    let name = format!("user-plugin-{}", 42);
    let id = ModuleId::from_string(name);
    assert_eq!(id.as_str(), "user-plugin-42");
    assert!(id.is_dynamic());
    assert!(!id.is_static());
}

#[test]
fn test_module_id_static() {
    let id = ModuleId::new("lang-rust");
    assert!(id.is_static());
    assert!(!id.is_dynamic());
}

#[test]
fn test_module_id_static_dynamic_equality() {
    // Static and dynamic IDs with same content should be equal
    let static_id = ModuleId::new("test-module");
    let dynamic_id = ModuleId::from_string("test-module".to_string());

    assert_eq!(static_id, dynamic_id);
    assert_eq!(static_id.as_str(), dynamic_id.as_str());
}

#[test]
fn test_module_id_from_traits() {
    // Test From<&'static str>
    let id1: ModuleId = "lang-rust".into();
    assert_eq!(id1.as_str(), "lang-rust");

    // Test From<String>
    let id2: ModuleId = String::from("lang-python").into();
    assert_eq!(id2.as_str(), "lang-python");
}
