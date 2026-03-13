use super::*;

#[test]
fn test_service_key_impl() {
    assert_eq!(CompositorKey::service_name(), "Compositor");
}

#[test]
fn test_equality() {
    assert_eq!(CompositorKey::Root, CompositorKey::Root);
}

#[test]
fn test_debug() {
    assert_eq!(format!("{:?}", CompositorKey::Root), "Root");
}
