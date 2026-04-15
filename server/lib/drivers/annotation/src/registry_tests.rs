use {super::*, reovim_kernel::api::v1::ServiceKey};

#[test]
fn test_annotation_source_key_new() {
    let key = AnnotationSourceKey::new("line_number");
    assert_eq!(key.0, "line_number");
}

#[test]
fn test_annotation_source_key_service_name() {
    assert_eq!(AnnotationSourceKey::service_name(), "AnnotationSource");
}

#[test]
fn test_annotation_source_key_equality() {
    let k1 = AnnotationSourceKey::new("a");
    let k2 = AnnotationSourceKey::new("a");
    let k3 = AnnotationSourceKey::new("b");
    assert_eq!(k1, k2);
    assert_ne!(k1, k3);
}

#[test]
fn test_annotation_source_key_clone() {
    let key = AnnotationSourceKey::new("test");
    let cloned = key.clone();
    assert_eq!(key, cloned);
}

#[test]
fn test_annotation_source_key_debug() {
    let key = AnnotationSourceKey::new("test");
    let debug = format!("{key:?}");
    assert!(debug.contains("test"));
}
