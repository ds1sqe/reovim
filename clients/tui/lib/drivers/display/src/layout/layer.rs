//! Nested layer system for compositing windows.
//!
//! This module provides types for the Hyprland-inspired nested compositor
//! architecture. Each layer is a self-contained mini-compositor with
//! three zones: Tiled, Float, and Overlay.
//!
//! # Layer Model
//!
//! ```text
//! Layer (Self-contained compositor)
//! ├── Overlay Zone (z: layer_z + 50-99) - Popups, tooltips, menus
//! ├── Float Zone   (z: layer_z + 10-49) - Floating windows
//! └── Tiled Zone   (z: layer_z + 0-9)   - Split windows (binary tree)
//! ```

use crate::{Rect, WindowId};

use super::view::{ColIndex, LineIndex};

/// Opacity threshold below which mouse clicks pass through to the layer below.
///
/// Layers with opacity below this value are considered "click-through" —
/// they are still rendered (dimmed) but do not capture mouse input.
pub const CLICK_THROUGH_THRESHOLD: f32 = 0.1;

/// Unique identifier for a layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LayerId(pub u16);

impl LayerId {
    /// Create a new layer ID.
    #[must_use]
    pub const fn new(id: u16) -> Self {
        Self(id)
    }

    /// Get the raw ID value.
    #[must_use]
    pub const fn as_u16(self) -> u16 {
        self.0
    }
}

/// Zone within a layer (each layer has three zones).
///
/// Zones determine rendering order within a layer:
/// - Tiled windows are rendered first (background)
/// - Floating windows are rendered above tiled
/// - Overlays are rendered on top of everything
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Zone {
    /// Tiled windows (z: `layer_base` + 0-9). Binary split tree.
    Tiled,
    /// Floating windows (z: `layer_base` + 10-49). Free positioning.
    Float,
    /// Overlay windows (z: `layer_base` + 50-99). Popups, menus.
    Overlay,
}

impl Zone {
    /// Z-order offset within layer's z-range.
    ///
    /// # Z-Order Computation
    ///
    /// ```text
    /// z_order = layer.z_base + zone.z_offset() + window_index
    ///
    /// Example for Layer 1 (z_base=100):
    ///   Tiled windows:   z=100, 101, 102...
    ///   Float windows:   z=110, 111, 112...
    ///   Overlay windows: z=150, 151, 152...
    /// ```
    #[must_use]
    pub const fn z_offset(self) -> u16 {
        match self {
            Self::Tiled => 0,
            Self::Float => 10,
            Self::Overlay => 50,
        }
    }
}

/// Z-order value for window stacking.
///
/// Lower values are rendered first (background), higher values on top.
/// Each layer has a base z-order, and zones/windows add offsets:
///
/// ```text
/// Layer 0 (z_base=0):   Tiled=0-9, Float=10-49, Overlay=50-99
/// Layer 1 (z_base=100): Tiled=100-109, Float=110-149, Overlay=150-199
/// ```
///
/// # Type Safety
///
/// Using a newtype prevents accidentally mixing z-order values with
/// other `u16` values like dimensions or positions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct ZOrder(u16);

impl ZOrder {
    /// Create a new z-order value.
    #[must_use]
    pub const fn new(value: u16) -> Self {
        Self(value)
    }

    /// Get the raw z-order value.
    #[must_use]
    pub const fn as_u16(self) -> u16 {
        self.0
    }

    /// Compute the base z-order for a layer.
    ///
    /// Each layer gets 100 z-order slots (0-99, 100-199, etc.).
    #[must_use]
    pub const fn layer_base(layer_id: LayerId) -> Self {
        Self(layer_id.as_u16() * 100)
    }

    /// Compute the final z-order for a window.
    ///
    /// # Arguments
    ///
    /// * `layer_base` - Base z-order for the layer
    /// * `zone` - Which zone the window is in
    /// * `index` - Window index within the zone (for stacking order)
    #[must_use]
    pub const fn for_window(layer_base: Self, zone: Zone, index: u16) -> Self {
        Self(layer_base.0 + zone.z_offset() + index)
    }

    /// Add an offset to the z-order.
    #[must_use]
    pub const fn offset(self, delta: u16) -> Self {
        Self(self.0 + delta)
    }
}

/// A Layer is a self-contained mini-compositor.
///
/// Each layer contains three zones (Tiled, Float, Overlay) and can be
/// stacked with other layers. Think: Hyprland workspaces that can overlay.
#[derive(Debug, Clone)]
pub struct Layer {
    /// Unique identifier.
    pub id: LayerId,
    /// Human-readable label for shortcuts (e.g., "main", "term", "1").
    pub label: String,
    /// Base z-order (layers stack: 0, 100, 200...).
    pub z_base: ZOrder,
    /// Screen bounds this layer occupies.
    pub bounds: Rect,
    /// Opacity (0.0 = transparent, 1.0 = opaque). Future use.
    pub opacity: f32,
    /// Whether this layer is visible.
    pub visible: bool,
}

impl Layer {
    /// Create a new layer with default settings.
    #[must_use]
    pub fn new(id: LayerId, label: impl Into<String>, z_base: ZOrder) -> Self {
        Self {
            id,
            label: label.into(),
            z_base,
            bounds: Rect::default(),
            opacity: 1.0,
            visible: true,
        }
    }

    /// Check if this layer is click-through.
    ///
    /// A layer is click-through when its opacity is below
    /// [`CLICK_THROUGH_THRESHOLD`]. Click-through layers are still
    /// rendered (dimmed) but do not capture mouse input — clicks
    /// pass through to the layer below.
    ///
    /// Keyboard input is NOT affected by click-through; it always
    /// goes to the focused layer regardless of opacity.
    #[must_use]
    pub fn is_click_through(&self) -> bool {
        self.opacity < CLICK_THROUGH_THRESHOLD
    }

    /// Calculate z-order for a window in a specific zone.
    ///
    /// # Arguments
    ///
    /// * `zone` - The zone the window belongs to
    /// * `index` - Window index within the zone (0, 1, 2...)
    ///
    /// # Returns
    ///
    /// Absolute z-order value for rendering.
    #[must_use]
    pub const fn z_for(&self, zone: Zone, index: u16) -> ZOrder {
        ZOrder::for_window(self.z_base, zone, index)
    }
}

/// Positioned window ready for rendering.
///
/// This is the output of the compositor's `arrange()` method.
/// Contains all information needed to render a window.
#[derive(Debug, Clone)]
pub struct WindowPlacement {
    /// The window being placed.
    pub window_id: WindowId,
    /// Layer containing this window.
    pub layer_id: LayerId,
    /// Zone within the layer.
    pub zone: Zone,
    /// Computed screen bounds.
    pub bounds: Rect,
    /// Computed z-order for rendering.
    pub z_order: ZOrder,
    /// Whether the window is currently visible.
    pub visible: bool,
    /// Whether the window can receive focus.
    pub focusable: bool,
    /// Layer opacity (0.0 = fully transparent, 1.0 = fully opaque).
    pub opacity: f32,
}

impl WindowPlacement {
    /// Create a new window placement.
    #[must_use]
    pub const fn new(
        window_id: WindowId,
        layer_id: LayerId,
        zone: Zone,
        bounds: Rect,
        z_order: ZOrder,
    ) -> Self {
        Self {
            window_id,
            layer_id,
            zone,
            bounds,
            z_order,
            visible: true,
            focusable: true,
            opacity: 1.0,
        }
    }

    /// Check if this placement is click-through.
    ///
    /// A window placement inherits click-through from its layer's opacity.
    /// When click-through, mouse clicks pass through to windows below.
    #[must_use]
    pub fn is_click_through(&self) -> bool {
        self.opacity < CLICK_THROUGH_THRESHOLD
    }
}

/// Anchor point for positioning overlays.
///
/// Overlays can be anchored to various reference points.
#[derive(Debug, Clone, Copy)]
#[allow(clippy::doc_markdown)] // Code blocks in doc comments
pub enum Anchor {
    /// Relative to cursor position in a window.
    Cursor {
        /// Window containing the cursor.
        window: WindowId,
        /// Line number (0-indexed).
        line: LineIndex,
        /// Column number (0-indexed).
        col: ColIndex,
    },
    /// Relative to a screen position.
    Screen {
        /// X coordinate (column).
        x: u16,
        /// Y coordinate (row).
        y: u16,
    },
    /// Centered on screen.
    Center,
    /// Below another window.
    Below(WindowId),
}

/// Constraints for overlay positioning.
///
/// Used when creating overlays to specify size preferences.
#[derive(Debug, Clone)]
pub struct OverlayConstraints {
    /// Anchor point for positioning.
    pub anchor: Anchor,
    /// Preferred width (if any).
    pub preferred_width: Option<u16>,
    /// Preferred height (if any).
    pub preferred_height: Option<u16>,
    /// Maximum allowed width.
    pub max_width: Option<u16>,
    /// Maximum allowed height.
    pub max_height: Option<u16>,
}

impl OverlayConstraints {
    /// Create constraints anchored to cursor position.
    #[must_use]
    pub const fn at_cursor(window: WindowId, line: LineIndex, col: ColIndex) -> Self {
        Self {
            anchor: Anchor::Cursor { window, line, col },
            preferred_width: None,
            preferred_height: None,
            max_width: None,
            max_height: None,
        }
    }

    /// Create constraints anchored to cursor position from raw values.
    #[must_use]
    pub const fn at_cursor_raw(window: WindowId, line: usize, col: usize) -> Self {
        Self {
            anchor: Anchor::Cursor {
                window,
                line: LineIndex::new(line),
                col: ColIndex::new(col),
            },
            preferred_width: None,
            preferred_height: None,
            max_width: None,
            max_height: None,
        }
    }

    /// Create constraints anchored to screen position.
    #[must_use]
    pub const fn at_position(x: u16, y: u16) -> Self {
        Self {
            anchor: Anchor::Screen { x, y },
            preferred_width: None,
            preferred_height: None,
            max_width: None,
            max_height: None,
        }
    }

    /// Create centered constraints.
    #[must_use]
    pub const fn centered() -> Self {
        Self {
            anchor: Anchor::Center,
            preferred_width: None,
            preferred_height: None,
            max_width: None,
            max_height: None,
        }
    }

    /// Set preferred dimensions.
    #[must_use]
    pub const fn with_size(mut self, width: u16, height: u16) -> Self {
        self.preferred_width = Some(width);
        self.preferred_height = Some(height);
        self
    }

    /// Set maximum dimensions.
    #[must_use]
    pub const fn with_max_size(mut self, width: u16, height: u16) -> Self {
        self.max_width = Some(width);
        self.max_height = Some(height);
        self
    }
}

/// Layer creation parameters.
#[derive(Debug, Clone)]
pub struct LayerConfig {
    /// Human-readable label for shortcuts.
    pub label: String,
    /// Screen bounds (None = fullscreen).
    pub bounds: Option<Rect>,
    /// Initial opacity (0.0-1.0).
    pub opacity: f32,
}

impl LayerConfig {
    /// Create a fullscreen layer configuration.
    #[must_use]
    pub fn fullscreen(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            bounds: None,
            opacity: 1.0,
        }
    }

    /// Create a layer with specific bounds.
    #[must_use]
    pub fn with_bounds(label: impl Into<String>, bounds: Rect) -> Self {
        Self {
            label: label.into(),
            bounds: Some(bounds),
            opacity: 1.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_zone_z_offset() {
        assert_eq!(Zone::Tiled.z_offset(), 0);
        assert_eq!(Zone::Float.z_offset(), 10);
        assert_eq!(Zone::Overlay.z_offset(), 50);
    }

    #[test]
    fn test_zone_ordering() {
        // Zones should be ordered: Tiled < Float < Overlay
        assert!(Zone::Tiled < Zone::Float);
        assert!(Zone::Float < Zone::Overlay);
    }

    #[test]
    fn test_layer_z_for() {
        let layer = Layer::new(LayerId::new(1), "main", ZOrder::new(100));

        // Tiled windows: 100, 101, 102...
        assert_eq!(layer.z_for(Zone::Tiled, 0), ZOrder::new(100));
        assert_eq!(layer.z_for(Zone::Tiled, 5), ZOrder::new(105));

        // Float windows: 110, 111, 112...
        assert_eq!(layer.z_for(Zone::Float, 0), ZOrder::new(110));
        assert_eq!(layer.z_for(Zone::Float, 3), ZOrder::new(113));

        // Overlay windows: 150, 151, 152...
        assert_eq!(layer.z_for(Zone::Overlay, 0), ZOrder::new(150));
        assert_eq!(layer.z_for(Zone::Overlay, 2), ZOrder::new(152));
    }

    #[test]
    fn test_layer_id() {
        let id = LayerId::new(42);
        assert_eq!(id.as_u16(), 42);
    }

    #[test]
    fn test_window_placement_new() {
        let window_id = WindowId::from_raw(1);
        let placement = WindowPlacement::new(
            window_id,
            LayerId::new(0),
            Zone::Tiled,
            Rect::new(0, 0, 80, 24),
            ZOrder::new(100),
        );

        assert_eq!(placement.window_id, window_id);
        assert_eq!(placement.zone, Zone::Tiled);
        assert_eq!(placement.z_order, ZOrder::new(100));
        assert!(placement.visible);
        assert!(placement.focusable);
        assert!((placement.opacity - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_click_through_threshold() {
        assert!((CLICK_THROUGH_THRESHOLD - 0.1).abs() < f32::EPSILON);
        // Layers below threshold should be click-through
        const { assert!(0.05 < CLICK_THROUGH_THRESHOLD) };
        // Layers at threshold should NOT be click-through
        assert!((CLICK_THROUGH_THRESHOLD - 0.1).abs() < f32::EPSILON);
    }

    #[test]
    fn test_z_order_layer_base() {
        assert_eq!(ZOrder::layer_base(LayerId::new(0)), ZOrder::new(0));
        assert_eq!(ZOrder::layer_base(LayerId::new(1)), ZOrder::new(100));
        assert_eq!(ZOrder::layer_base(LayerId::new(2)), ZOrder::new(200));
    }

    #[test]
    fn test_z_order_for_window() {
        let base = ZOrder::layer_base(LayerId::new(1));

        // Tiled: base + 0 + index
        assert_eq!(ZOrder::for_window(base, Zone::Tiled, 0), ZOrder::new(100));
        assert_eq!(ZOrder::for_window(base, Zone::Tiled, 5), ZOrder::new(105));

        // Float: base + 10 + index
        assert_eq!(ZOrder::for_window(base, Zone::Float, 0), ZOrder::new(110));
        assert_eq!(ZOrder::for_window(base, Zone::Float, 3), ZOrder::new(113));

        // Overlay: base + 50 + index
        assert_eq!(ZOrder::for_window(base, Zone::Overlay, 0), ZOrder::new(150));
    }

    #[test]
    fn test_z_order_comparison() {
        let z1 = ZOrder::new(100);
        let z2 = ZOrder::new(150);

        assert!(z1 < z2);
        assert!(z2 > z1);
        assert_eq!(z1, ZOrder::new(100));
    }

    #[test]
    fn test_overlay_constraints_builder() {
        let constraints = OverlayConstraints::centered()
            .with_size(40, 10)
            .with_max_size(60, 20);

        assert!(matches!(constraints.anchor, Anchor::Center));
        assert_eq!(constraints.preferred_width, Some(40));
        assert_eq!(constraints.preferred_height, Some(10));
        assert_eq!(constraints.max_width, Some(60));
        assert_eq!(constraints.max_height, Some(20));
    }

    #[test]
    fn test_layer_config_fullscreen() {
        let config = LayerConfig::fullscreen("main");
        assert_eq!(config.label, "main");
        assert!(config.bounds.is_none());
        assert!((config.opacity - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_z_order_as_u16() {
        let z = ZOrder::new(42);
        assert_eq!(z.as_u16(), 42);

        let z_zero = ZOrder::new(0);
        assert_eq!(z_zero.as_u16(), 0);

        let z_max = ZOrder::new(u16::MAX);
        assert_eq!(z_max.as_u16(), u16::MAX);
    }

    #[test]
    fn test_z_order_offset() {
        let z = ZOrder::new(100);
        let shifted = z.offset(25);
        assert_eq!(shifted.as_u16(), 125);

        let z_zero = ZOrder::new(0);
        assert_eq!(z_zero.offset(0).as_u16(), 0);
        assert_eq!(z_zero.offset(50).as_u16(), 50);
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_overlay_constraints_at_cursor() {
        let window = WindowId::from_raw(5);
        let line = LineIndex::new(10);
        let col = ColIndex::new(20);
        let constraints = OverlayConstraints::at_cursor(window, line, col);

        match constraints.anchor {
            Anchor::Cursor {
                window: w,
                line: l,
                col: c,
            } => {
                assert_eq!(w, window);
                assert_eq!(l, line);
                assert_eq!(c, col);
            }
            _ => panic!("Expected Anchor::Cursor"),
        }
        assert_eq!(constraints.preferred_width, None);
        assert_eq!(constraints.preferred_height, None);
        assert_eq!(constraints.max_width, None);
        assert_eq!(constraints.max_height, None);
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_overlay_constraints_at_cursor_raw() {
        let window = WindowId::from_raw(3);
        let constraints = OverlayConstraints::at_cursor_raw(window, 7, 15);

        match constraints.anchor {
            Anchor::Cursor {
                window: w,
                line,
                col,
            } => {
                assert_eq!(w, window);
                assert_eq!(line, LineIndex::new(7));
                assert_eq!(col, ColIndex::new(15));
            }
            _ => panic!("Expected Anchor::Cursor"),
        }
        assert_eq!(constraints.preferred_width, None);
        assert_eq!(constraints.preferred_height, None);
        assert_eq!(constraints.max_width, None);
        assert_eq!(constraints.max_height, None);
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_overlay_constraints_at_position() {
        let constraints = OverlayConstraints::at_position(30, 10);

        match constraints.anchor {
            Anchor::Screen { x, y } => {
                assert_eq!(x, 30);
                assert_eq!(y, 10);
            }
            _ => panic!("Expected Anchor::Screen"),
        }
        assert_eq!(constraints.preferred_width, None);
        assert_eq!(constraints.preferred_height, None);
        assert_eq!(constraints.max_width, None);
        assert_eq!(constraints.max_height, None);
    }

    #[test]
    fn test_layer_config_with_bounds() {
        let bounds = Rect::new(10, 5, 60, 20);
        let config = LayerConfig::with_bounds("sidebar", bounds);

        assert_eq!(config.label, "sidebar");
        assert_eq!(config.bounds, Some(bounds));
        assert!((config.opacity - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_layer_is_click_through_fully_opaque() {
        let layer = Layer::new(LayerId::new(0), "main", ZOrder::new(0));
        assert!(!layer.is_click_through());
    }

    #[test]
    fn test_layer_is_click_through_at_threshold() {
        let mut layer = Layer::new(LayerId::new(0), "main", ZOrder::new(0));
        layer.opacity = CLICK_THROUGH_THRESHOLD;
        // At threshold (0.1), NOT click-through (< not <=)
        assert!(!layer.is_click_through());
    }

    #[test]
    fn test_layer_is_click_through_below_threshold() {
        let mut layer = Layer::new(LayerId::new(0), "main", ZOrder::new(0));
        layer.opacity = 0.05;
        assert!(layer.is_click_through());
    }

    #[test]
    fn test_layer_is_click_through_zero_opacity() {
        let mut layer = Layer::new(LayerId::new(0), "main", ZOrder::new(0));
        layer.opacity = 0.0;
        assert!(layer.is_click_through());
    }

    #[test]
    fn test_placement_is_click_through_default() {
        let placement = WindowPlacement::new(
            WindowId::from_raw(1),
            LayerId::new(0),
            Zone::Tiled,
            Rect::new(0, 0, 80, 24),
            ZOrder::new(0),
        );
        // Default opacity is 1.0 — not click-through
        assert!(!placement.is_click_through());
    }

    #[test]
    fn test_placement_is_click_through_transparent() {
        let mut placement = WindowPlacement::new(
            WindowId::from_raw(1),
            LayerId::new(0),
            Zone::Tiled,
            Rect::new(0, 0, 80, 24),
            ZOrder::new(0),
        );
        placement.opacity = 0.05;
        assert!(placement.is_click_through());
    }
}
