//! Overlay compositor for z-order rendering

use {
    super::{Overlay, OverlayBounds},
    crate::highlight::{ColorMode, Theme},
};

/// Entry for a registered overlay
struct OverlayEntry<'a> {
    overlay: &'a dyn Overlay,
    z_order: u16,
}

/// Compositor that manages z-order rendering of overlays
///
/// The compositor collects visible overlays and renders them in z-order,
/// with higher z-order overlays drawn on top.
pub struct OverlayCompositor<'a> {
    entries: Vec<OverlayEntry<'a>>,
}

impl<'a> OverlayCompositor<'a> {
    /// Create a new empty compositor
    #[must_use]
    #[allow(clippy::missing_const_for_fn)] // Vec::new() not const-stable
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Add an overlay to the compositor
    ///
    /// Only adds the overlay if it's currently visible.
    pub fn add(&mut self, overlay: &'a dyn Overlay) {
        if overlay.is_visible() {
            self.entries.push(OverlayEntry {
                overlay,
                z_order: overlay.z_order(),
            });
        }
    }

    /// Clear all registered overlays
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Check if any overlays are registered
    #[must_use]
    #[allow(clippy::missing_const_for_fn)] // Vec::is_empty not const-stable
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Get the number of registered overlays
    #[must_use]
    #[allow(clippy::missing_const_for_fn)] // Vec::len not const-stable
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Render all overlays in z-order
    ///
    /// Returns positioned lines ready for terminal output.
    /// Lines are sorted by z-order (lower first, so higher z-order draws on top).
    #[must_use]
    pub fn render(&self, theme: &Theme, color_mode: ColorMode) -> Vec<(String, u16, u16)> {
        // Sort entries by z-order (lower first)
        let mut sorted: Vec<_> = self.entries.iter().collect();
        sorted.sort_by_key(|e| e.z_order);

        // Collect all rendered lines
        let mut lines = Vec::new();
        for entry in sorted {
            lines.extend(entry.overlay.render(theme, color_mode));
        }

        lines
    }

    /// Get bounds of all visible overlays
    ///
    /// Useful for determining which screen regions need updating.
    #[must_use]
    pub fn get_all_bounds(&self, screen_width: u16, screen_height: u16) -> Vec<OverlayBounds> {
        self.entries
            .iter()
            .map(|e| e.overlay.compute_bounds(screen_width, screen_height))
            .collect()
    }

    /// Check if any overlay captures input
    #[must_use]
    pub fn any_captures_input(&self) -> bool {
        self.entries.iter().any(|e| e.overlay.captures_input())
    }

    /// Get the topmost overlay that captures input (highest z-order)
    #[must_use]
    pub fn topmost_input_captor(&self) -> Option<&'a dyn Overlay> {
        self.entries
            .iter()
            .filter(|e| e.overlay.captures_input())
            .max_by_key(|e| e.z_order)
            .map(|e| e.overlay)
    }
}

impl Default for OverlayCompositor<'_> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::{
            highlight::{ColorMode, Theme},
            overlay::{OverlayGeometry, OverlayRender},
        },
    };

    // Test overlay implementation
    struct TestOverlay {
        id: &'static str,
        z: u16,
        visible: bool,
        captures: bool,
    }

    impl OverlayGeometry for TestOverlay {
        fn compute_bounds(&self, _sw: u16, _sh: u16) -> OverlayBounds {
            OverlayBounds::new(0, 0, 10, 5)
        }
    }

    impl OverlayRender for TestOverlay {
        fn render(&self, _theme: &Theme, _color_mode: ColorMode) -> Vec<(String, u16, u16)> {
            vec![(format!("overlay-{}", self.id), 0, 0)]
        }
    }

    impl Overlay for TestOverlay {
        fn overlay_id(&self) -> &'static str {
            self.id
        }

        fn z_order(&self) -> u16 {
            self.z
        }

        fn captures_input(&self) -> bool {
            self.captures
        }

        fn is_visible(&self) -> bool {
            self.visible
        }
    }

    #[test]
    fn test_compositor_add_visible() {
        let overlay = TestOverlay {
            id: "test",
            z: 100,
            visible: true,
            captures: true,
        };
        let mut compositor = OverlayCompositor::new();
        compositor.add(&overlay);
        assert_eq!(compositor.len(), 1);
    }

    #[test]
    fn test_compositor_skip_invisible() {
        let overlay = TestOverlay {
            id: "test",
            z: 100,
            visible: false,
            captures: true,
        };
        let mut compositor = OverlayCompositor::new();
        compositor.add(&overlay);
        assert!(compositor.is_empty());
    }

    #[test]
    fn test_render_z_order() {
        let low = TestOverlay {
            id: "low",
            z: 100,
            visible: true,
            captures: false,
        };
        let high = TestOverlay {
            id: "high",
            z: 200,
            visible: true,
            captures: false,
        };

        let mut compositor = OverlayCompositor::new();
        // Add in reverse order to test sorting
        compositor.add(&high);
        compositor.add(&low);

        let theme = Theme::default();
        let lines = compositor.render(&theme, ColorMode::TrueColor);

        // Low z-order should render first (be at lower index)
        assert_eq!(lines.len(), 2);
        assert!(lines[0].0.contains("low"));
        assert!(lines[1].0.contains("high"));
    }

    #[test]
    fn test_topmost_input_captor() {
        let low = TestOverlay {
            id: "low",
            z: 100,
            visible: true,
            captures: true,
        };
        let high = TestOverlay {
            id: "high",
            z: 200,
            visible: true,
            captures: true,
        };
        let highest_no_capture = TestOverlay {
            id: "highest",
            z: 300,
            visible: true,
            captures: false,
        };

        let mut compositor = OverlayCompositor::new();
        compositor.add(&low);
        compositor.add(&high);
        compositor.add(&highest_no_capture);

        let topmost = compositor.topmost_input_captor();
        assert!(topmost.is_some());
        assert_eq!(topmost.unwrap().overlay_id(), "high");
    }
}
