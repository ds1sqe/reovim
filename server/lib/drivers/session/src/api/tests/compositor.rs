
use super::*;


#[test]
fn test_compositor_error_display() {
    assert_eq!(CompositorError::CannotCloseLastWindow.to_string(), "cannot close last window");
    assert_eq!(
        CompositorError::NoNeighbor(NavigateDirection::Right).to_string(),
        "no window Right"
    );
    assert_eq!(CompositorError::NoActiveLayer.to_string(), "no active layer");
    assert_eq!(
        CompositorError::WindowNotFound(WindowId::from_raw(42)).to_string(),
        "window 42 not found"
    );
    assert_eq!(
        CompositorError::LayerNotFound(LayerId::new(5)).to_string(),
        "layer 5 not found"
    );
}

#[test]
fn test_compositor_error_display_all_directions() {
    assert_eq!(
        CompositorError::NoNeighbor(NavigateDirection::Left).to_string(),
        "no window Left"
    );
    assert_eq!(CompositorError::NoNeighbor(NavigateDirection::Up).to_string(), "no window Up");
    assert_eq!(
        CompositorError::NoNeighbor(NavigateDirection::Down).to_string(),
        "no window Down"
    );
}

#[test]
fn test_compositor_error_not_enough_room() {
    assert_eq!(CompositorError::NotEnoughRoom.to_string(), "not enough room to split");
}

#[test]
fn test_compositor_error_cannot_resize_at_edge() {
    assert_eq!(CompositorError::CannotResizeAtEdge.to_string(), "cannot resize at edge");
}

#[test]
fn test_compositor_error_no_focused_window() {
    assert_eq!(CompositorError::NoFocusedWindow.to_string(), "no focused window");
}

#[test]
fn test_compositor_error_is_std_error() {
    let err: Box<dyn std::error::Error> = Box::new(CompositorError::CannotCloseLastWindow);
    assert_eq!(err.to_string(), "cannot close last window");
}

#[test]
fn test_compositor_error_debug() {
    let err = CompositorError::NoActiveLayer;
    let debug = format!("{err:?}");
    assert!(debug.contains("NoActiveLayer"));
}

#[test]
fn test_compositor_error_clone() {
    let err = CompositorError::NotEnoughRoom;
    let cloned = err.clone();
    assert_eq!(err, cloned);
}

#[test]
fn test_compositor_error_eq() {
    assert_eq!(CompositorError::NoActiveLayer, CompositorError::NoActiveLayer);
    assert_ne!(CompositorError::NoActiveLayer, CompositorError::NoFocusedWindow);
}

#[test]
fn test_compositor_error_no_tab_pages() {
    assert_eq!(CompositorError::NoTabPages.to_string(), "no tab pages");
}

#[test]
fn test_compositor_error_cannot_close_last_tab() {
    assert_eq!(CompositorError::CannotCloseLastTab.to_string(), "cannot close last tab");
}

#[test]
fn test_compositor_error_tab_not_found() {
    assert_eq!(CompositorError::TabNotFound(TabId::from_raw(3)).to_string(), "tab 3 not found");
}