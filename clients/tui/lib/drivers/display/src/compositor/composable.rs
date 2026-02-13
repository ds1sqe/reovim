//! Composable trait and identifier types.

use super::{Bounds, ZGroup, ZOrder};

// Local types from this crate
pub use crate::{frame::FrameBuffer, highlight::Style};

/// Unique identifier for composable elements.
///
/// Each composable element has a unique ID that allows it to be
/// registered, retrieved, and manipulated in the compositor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ComposableId {
    /// Base layer (background)
    Base,
    /// Tab line at the top
    TabLine,
    /// Status line at the bottom
    StatusLine,
    /// Editor window by index
    Window(usize),
    /// Floating window by index
    FloatingWindow(usize),
    /// Custom plugin identifier
    Custom(&'static str),
}

impl ComposableId {
    /// Get the default z-group for this ID type.
    ///
    /// This determines the initial z-order group when a composable
    /// is registered without an explicit z-order.
    #[must_use]
    pub const fn default_group(&self) -> ZGroup {
        match self {
            Self::Base | Self::TabLine | Self::StatusLine => ZGroup::Base,
            Self::Window(_) | Self::Custom(_) => ZGroup::Editor,
            Self::FloatingWindow(_) => ZGroup::Floating,
        }
    }
}

/// Trait for elements that can be composited by the compositor.
///
/// Composables are renderable UI elements with z-ordering support.
/// They can capture keyboard input and provide cursor positions.
pub trait Composable: std::fmt::Debug + Send + Sync {
    /// Get the unique identifier for this composable.
    fn id(&self) -> ComposableId;

    /// Get the current z-order.
    fn z_order(&self) -> ZOrder;

    /// Set the z-order.
    ///
    /// Called by the compositor when `bring_to_front`/`send_to_back`/`set_z_order`
    /// operations are performed.
    fn set_z_order(&mut self, z: ZOrder);

    /// Check if this composable is visible.
    ///
    /// Invisible composables are skipped during rendering.
    fn is_visible(&self) -> bool;

    /// Compute bounds based on screen dimensions.
    ///
    /// The bounds may depend on the screen size (e.g., for full-screen elements
    /// or elements positioned relative to the screen edges).
    fn bounds(&self, screen_width: u16, screen_height: u16) -> Bounds;

    /// Render this composable to the frame buffer.
    ///
    /// # Arguments
    ///
    /// * `buffer` - The frame buffer to render to
    /// * `default_style` - The default style to use for unstyled content
    fn render(&self, buffer: &mut FrameBuffer, default_style: &Style);

    /// Check if this composable captures keyboard input.
    ///
    /// When true, this composable can be the keyboard target.
    /// Default is true.
    fn captures_keyboard(&self) -> bool {
        true
    }

    /// Get the cursor position if this composable owns the cursor.
    ///
    /// Returns the (x, y) position in screen coordinates, or None if
    /// this composable doesn't own the cursor.
    fn cursor_position(&self) -> Option<(u16, u16)> {
        None
    }
}

#[cfg(test)]
mod tests {
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
}
