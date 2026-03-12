use super::*;

#[test]
fn test_composable_id_default_group() {
    assert_eq!(ComposableId::Base.default_group(), ZGroup::Base);
    assert_eq!(ComposableId::TabLine.default_group(), ZGroup::Base);
    assert_eq!(ComposableId::StatusLine.default_group(), ZGroup::Base);
    assert_eq!(ComposableId::Window(0).default_group(), ZGroup::Editor);
    assert_eq!(ComposableId::FloatingWindow(0).default_group(), ZGroup::Floating);
    assert_eq!(ComposableId::Custom("test").default_group(), ZGroup::Editor);
}

#[test]
fn test_composable_id_equality() {
    assert_eq!(ComposableId::Window(0), ComposableId::Window(0));
    assert_ne!(ComposableId::Window(0), ComposableId::Window(1));
    assert_ne!(ComposableId::Window(0), ComposableId::FloatingWindow(0));
}

#[test]
fn test_composable_id_ordering() {
    // Base types come first
    assert!(ComposableId::Base < ComposableId::TabLine);
    assert!(ComposableId::TabLine < ComposableId::StatusLine);
    // Window types come after base types
    assert!(ComposableId::StatusLine < ComposableId::Window(0));
    assert!(ComposableId::Window(0) < ComposableId::Window(1));
}

// =========================================================================
// Coverage tests for default trait methods (lines 83-85, 91-93)
// =========================================================================

/// Minimal Composable implementor that relies on default methods.
#[derive(Debug)]
struct MinimalComposable;

#[cfg_attr(coverage_nightly, coverage(off))]
impl Composable for MinimalComposable {
    fn id(&self) -> ComposableId {
        ComposableId::Window(0)
    }
    fn z_order(&self) -> ZOrder {
        ZOrder::default()
    }
    fn set_z_order(&mut self, _z: ZOrder) {}
    fn is_visible(&self) -> bool {
        true
    }
    fn bounds(&self, _w: u16, _h: u16) -> Bounds {
        Bounds::new(0, 0, 10, 10)
    }
    fn render(
        &self,
        _buffer: &mut crate::frame::FrameBuffer,
        _style: &crate::highlight::Style,
    ) {
    }
    // captures_keyboard and cursor_position use defaults
}

/// Test default `captures_keyboard()` returns true (lines 83-85).
#[test]
fn test_composable_default_captures_keyboard() {
    let c = MinimalComposable;
    assert!(c.captures_keyboard());
}

/// Test default `cursor_position()` returns `None` (lines 91-93).
#[test]
fn test_composable_default_cursor_position() {
    let c = MinimalComposable;
    assert_eq!(c.cursor_position(), None);
}
