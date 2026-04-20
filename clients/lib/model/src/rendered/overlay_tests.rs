use super::*;

fn test_overlay(id: &str, kind: &str) -> LogicalOverlay {
    LogicalOverlay::new(id, kind)
}

fn rendered(id: &str, x: u16, y: u16, w: u16, h: u16) -> RenderedOverlay {
    RenderedOverlay::new(test_overlay(id, "test"), ScreenPosition::new(x, y), Size::new(w, h))
}

#[test]
fn test_rendered_overlay_new() {
    let overlay = rendered("test-1", 10, 20, 100, 50);
    assert_eq!(overlay.id(), "test-1");
    assert_eq!(overlay.position, ScreenPosition::new(10, 20));
    assert_eq!(overlay.size, Size::new(100, 50));
    assert!(!overlay.is_modal);
}

#[test]
fn test_rendered_overlay_bounds() {
    let overlay = rendered("test", 10, 20, 100, 50);
    let bounds = overlay.bounds();
    assert_eq!(bounds.x, 10);
    assert_eq!(bounds.y, 20);
    assert_eq!(bounds.width, 100);
    assert_eq!(bounds.height, 50);
}

#[test]
fn test_rendered_overlay_modal() {
    let overlay = rendered("modal", 0, 0, 80, 24).as_modal();
    assert!(overlay.is_modal);
}

#[test]
fn test_overlay_stack_push_remove() {
    let mut stack = OverlayStack::new();
    assert!(stack.is_empty());

    stack.push(rendered("a", 0, 0, 10, 10));
    stack.push(rendered("b", 0, 0, 10, 10));
    assert_eq!(stack.len(), 2);

    assert!(stack.remove("a"));
    assert_eq!(stack.len(), 1);
    assert!(!stack.remove("a")); // Already removed
}

#[test]
fn test_overlay_stack_z_order() {
    let mut stack = OverlayStack::new();
    stack.push(rendered("a", 0, 0, 10, 10));
    stack.push(rendered("b", 0, 0, 10, 10));
    stack.push(rendered("c", 0, 0, 10, 10));

    let ordered = stack.in_z_order();
    assert_eq!(ordered[0].id(), "a");
    assert_eq!(ordered[1].id(), "b");
    assert_eq!(ordered[2].id(), "c");

    // Verify z-indices are ascending
    assert!(ordered[0].z_index < ordered[1].z_index);
    assert!(ordered[1].z_index < ordered[2].z_index);
}

#[test]
fn test_overlay_stack_has_modal() {
    let mut stack = OverlayStack::new();
    stack.push(rendered("a", 0, 0, 10, 10));
    assert!(!stack.has_modal());

    stack.push(rendered("modal", 0, 0, 80, 24).as_modal());
    assert!(stack.has_modal());
}

#[test]
fn test_overlay_stack_topmost_modal() {
    let mut stack = OverlayStack::new();
    stack.push(rendered("modal-1", 0, 0, 80, 24).as_modal());
    stack.push(rendered("normal", 0, 0, 10, 10));
    stack.push(rendered("modal-2", 0, 0, 80, 24).as_modal());

    let topmost = stack.topmost_modal().unwrap();
    assert_eq!(topmost.id(), "modal-2");
}

#[test]
fn test_overlay_stack_at_position() {
    let mut stack = OverlayStack::new();
    stack.push(rendered("a", 0, 0, 20, 20));
    stack.push(rendered("b", 10, 10, 20, 20));

    // Position only in "a"
    let hit = stack.at_position(ScreenPosition::new(5, 5));
    assert_eq!(hit.unwrap().id(), "a");

    // Position in both - should get "b" (higher z-index)
    let hit = stack.at_position(ScreenPosition::new(15, 15));
    assert_eq!(hit.unwrap().id(), "b");

    // Position outside both
    let hit = stack.at_position(ScreenPosition::new(50, 50));
    assert!(hit.is_none());
}

#[test]
fn test_overlay_stack_get() {
    let mut stack = OverlayStack::new();
    stack.push(rendered("test", 0, 0, 10, 10));

    assert!(stack.get("test").is_some());
    assert!(stack.get("nonexistent").is_none());
}

#[test]
fn test_rendered_overlay_with_z_index() {
    let overlay = rendered("test", 0, 0, 10, 10).with_z_index(42);
    assert_eq!(overlay.z_index, 42);
}

#[test]
fn test_rendered_overlay_kind() {
    let overlay = rendered("test", 0, 0, 10, 10);
    assert_eq!(overlay.kind(), "test");
}

#[test]
fn test_overlay_stack_get_mut() {
    let mut stack = OverlayStack::new();
    stack.push(rendered("test", 0, 0, 10, 10));

    let entry = stack.get_mut("test");
    assert!(entry.is_some());
    entry.unwrap().is_modal = true;
    assert!(stack.get("test").unwrap().is_modal);

    assert!(stack.get_mut("nonexistent").is_none());
}

#[test]
fn test_overlay_stack_default() {
    let stack = OverlayStack::default();
    assert!(stack.is_empty());
}

#[test]
fn test_overlay_stack_topmost_modal_none() {
    let mut stack = OverlayStack::new();
    stack.push(rendered("normal", 0, 0, 10, 10));
    assert!(stack.topmost_modal().is_none());
}

#[test]
fn test_overlay_stack_push_preserves_higher_z() {
    let mut stack = OverlayStack::new();
    let high_z = rendered("high", 0, 0, 10, 10).with_z_index(100);
    stack.push(high_z);
    assert_eq!(stack.get("high").unwrap().z_index, 100);
}

#[test]
fn test_overlay_stack_clear() {
    let mut stack = OverlayStack::new();
    stack.push(rendered("a", 0, 0, 10, 10));
    stack.push(rendered("b", 0, 0, 10, 10));
    assert_eq!(stack.len(), 2);

    stack.clear();
    assert!(stack.is_empty());
}
