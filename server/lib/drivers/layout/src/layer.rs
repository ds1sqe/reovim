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
#[path = "layer_tests.rs"]
mod tests;
