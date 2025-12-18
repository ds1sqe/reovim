//! Z-layer system for compositor-based rendering
//!
//! Each renderable component implements the `Layer` trait,
//! allowing the compositor to render them in proper z-order.

use crate::{
    frame::FrameBuffer,
    highlight::{ColorMode, Theme},
};

/// Z-order constants for rendering layers
/// Higher values render on top (occlude lower values)
pub mod z_order {
    /// Base layer: tab line, status line (always visible)
    pub const BASE: u8 = 0;
    /// Explorer sidebar
    pub const EXPLORER: u8 = 1;
    /// Editor windows (text content)
    pub const EDITOR: u8 = 2;
    /// Leap labels (jump targets)
    pub const LEAP: u8 = 3;
    /// Completion popup
    pub const COMPLETION: u8 = 4;
    /// Which-key hint panel
    pub const WHICH_KEY: u8 = 5;
    /// Telescope fuzzy finder (full-screen overlay)
    pub const TELESCOPE: u8 = 6;
    /// Settings menu (full-screen overlay)
    pub const SETTINGS_MENU: u8 = 7;
}

/// Bounds of a layer (x, y, width, height)
#[derive(Debug, Clone, Copy, Default)]
pub struct LayerBounds {
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub height: u16,
}

impl LayerBounds {
    /// Create new bounds
    #[must_use]
    pub const fn new(x: u16, y: u16, width: u16, height: u16) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    /// Create full-screen bounds
    #[must_use]
    pub const fn full_screen(width: u16, height: u16) -> Self {
        Self {
            x: 0,
            y: 0,
            width,
            height,
        }
    }

    /// Check if this bounds contains a point
    #[must_use]
    pub const fn contains(&self, px: u16, py: u16) -> bool {
        px >= self.x && px < self.x + self.width && py >= self.y && py < self.y + self.height
    }
}

/// A renderable layer with bounds and z-order
///
/// Layers are rendered to the frame buffer in z-order.
/// Higher z-order layers occlude (overwrite) lower layers.
pub trait Layer {
    /// Z-order priority (higher draws on top)
    fn z_order(&self) -> u8;

    /// Whether this layer should render
    fn is_visible(&self) -> bool;

    /// Bounds of this layer on screen
    fn bounds(&self) -> LayerBounds;

    /// Render this layer's content to the frame buffer
    ///
    /// Implementations should write cells within their bounds.
    /// The compositor ensures higher z-order layers overwrite lower ones.
    fn render_to_buffer(&self, buffer: &mut FrameBuffer, theme: &Theme, color_mode: ColorMode);

    /// Cursor position if this layer owns the cursor
    ///
    /// Returns `Some((x, y))` if this layer should position the cursor,
    /// `None` otherwise. The highest visible layer with a cursor wins.
    fn cursor_position(&self) -> Option<(u16, u16)> {
        None
    }
}
