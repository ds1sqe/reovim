//! Tests for Projection types.

use super::projection::*;

#[test]
fn projection_tag_from_str() {
    let tag = ProjectionTag::from("text.cursor");
    assert_eq!(tag.as_str(), "text.cursor");
}

#[test]
fn projection_tag_display() {
    let tag = ProjectionTag::new("mesh.camera");
    assert_eq!(tag.to_string(), "mesh.camera");
}

#[test]
fn projection_tag_equality() {
    let a = ProjectionTag::from("text.mode");
    let b = ProjectionTag::from("text.mode");
    let c = ProjectionTag::from("text.cursor");
    assert_eq!(a, b);
    assert_ne!(a, c);
}

#[test]
fn domain_id_equality() {
    let a = DomainId(1);
    let b = DomainId(1);
    let c = DomainId(2);
    assert_eq!(a, b);
    assert_ne!(a, c);
}

#[test]
fn projection_persistent() {
    let proj = Projection {
        tag: ProjectionTag::from("text.cursor"),
        domain_id: DomainId(1),
        window_id: None,
        payload: vec![1, 2, 3],
        display: Some("line 1, col 5".to_string()),
        delivery: ProjectionDelivery::Persistent,
    };
    assert_eq!(proj.delivery, ProjectionDelivery::Persistent);
    assert_eq!(proj.payload, vec![1, 2, 3]);
}

#[test]
fn projection_transient() {
    let proj = Projection {
        tag: ProjectionTag::from("platform.haptic"),
        domain_id: DomainId(1),
        window_id: None,
        payload: vec![0xFF],
        display: None,
        delivery: ProjectionDelivery::Transient,
    };
    assert_eq!(proj.delivery, ProjectionDelivery::Transient);
    assert!(proj.display.is_none());
}

#[test]
fn projection_with_window_id() {
    use reovim_kernel::api::v1::WindowId;
    let wid = WindowId::from_raw(42);
    let proj = Projection {
        tag: ProjectionTag::from("text.viewport"),
        domain_id: DomainId(1),
        window_id: Some(wid),
        payload: vec![],
        display: None,
        delivery: ProjectionDelivery::Persistent,
    };
    assert_eq!(proj.window_id, Some(WindowId::from_raw(42)));
}
