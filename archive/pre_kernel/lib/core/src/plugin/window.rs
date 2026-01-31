//! Unified plugin window system
//!
//! Provides a single trait for all plugin UI rendering, replacing the previous
//! separate `WindowProvider`, `OverlayRenderer`, and `RenderContext` systems.
//!
//! # Example
//!
//! ```ignore
//! use reovim_core::plugin::{EditorContext, PluginStateRegistry, PluginWindow, WindowConfig, Rect};
//!
//! struct MyWindow;
//!
//! impl PluginWindow for MyWindow {
//!     fn window_config(
//!         &self,
//!         state: &Arc<PluginStateRegistry>,
//!         ctx: &EditorContext,
//!     ) -> Option<WindowConfig> {
//!         let (x, y, w, h) = ctx.floating(0.8, 0.6);
//!         Some(WindowConfig {
//!             bounds: Rect { x, y, width: w, height: h },
//!             z_order: 200,
//!             visible: true,
//!         })
//!     }
//!
//!     fn render(
//!         &self,
//!         state: &Arc<PluginStateRegistry>,
//!         ctx: &EditorContext,
//!         buffer: &mut FrameBuffer,
//!         bounds: Rect,
//!         theme: &Theme,
//!     ) {
//!         // Render content into bounds
//!     }
//! }
//! ```

use std::sync::Arc;

use crate::{frame::FrameBuffer, highlight::Theme};

use super::{EditorContext, PluginStateRegistry};

/// Simple rectangle for window bounds
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    /// X position (column)
    pub x: u16,
    /// Y position (row)
    pub y: u16,
    /// Width in columns
    pub width: u16,
    /// Height in rows
    pub height: u16,
}

impl Rect {
    /// Create a new rectangle
    #[must_use]
    pub const fn new(x: u16, y: u16, width: u16, height: u16) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    /// Create from tuple (x, y, width, height)
    #[must_use]
    pub const fn from_tuple(tuple: (u16, u16, u16, u16)) -> Self {
        Self {
            x: tuple.0,
            y: tuple.1,
            width: tuple.2,
            height: tuple.3,
        }
    }

    /// Check if the rectangle has zero area
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.width == 0 || self.height == 0
    }

    /// Check if a point is inside the rectangle
    #[must_use]
    pub const fn contains(&self, x: u16, y: u16) -> bool {
        x >= self.x && x < self.x + self.width && y >= self.y && y < self.y + self.height
    }
}

/// Window configuration returned by plugins
#[derive(Debug, Clone, Copy)]
pub struct WindowConfig {
    /// Window bounds (position and size)
    pub bounds: Rect,
    /// Z-order for rendering (higher = on top)
    /// Recommended values:
    /// - 100: Editor windows (buffers)
    /// - 150: Sidebars (explorer)
    /// - 200: Dropdowns (completion)
    /// - 300: Floating (telescope, picker)
    /// - 400: Modal (settings, dialogs)
    pub z_order: u32,
    /// Whether the window is visible
    pub visible: bool,
}

impl WindowConfig {
    /// Create a new window config
    #[must_use]
    pub const fn new(bounds: Rect, z_order: u32, visible: bool) -> Self {
        Self {
            bounds,
            z_order,
            visible,
        }
    }
}

/// Unified trait for plugins that render UI
///
/// This trait replaces the previous `WindowProvider`, `OverlayRenderer`, and
/// `RenderContext` systems with a single unified interface.
///
/// # Lifecycle
///
/// 1. `window_config()` is called first to get position, size, z-order, visibility
/// 2. If visible, `render()` is called with the bounds from config
/// 3. Windows are rendered in z-order (lowest first, highest on top)
pub trait PluginWindow: Send + Sync {
    /// Get window configuration (position, size, visibility, z-order)
    ///
    /// Return `None` to skip rendering entirely (equivalent to visible=false).
    /// Return `Some(config)` with `visible=false` to reserve space but not render.
    fn window_config(
        &self,
        state: &Arc<PluginStateRegistry>,
        ctx: &EditorContext,
    ) -> Option<WindowConfig>;

    /// Render content into the window bounds
    ///
    /// The `bounds` parameter matches `window_config().bounds` and defines
    /// the area where this window should render. Content outside bounds
    /// will be clipped.
    fn render(
        &self,
        state: &Arc<PluginStateRegistry>,
        ctx: &EditorContext,
        buffer: &mut FrameBuffer,
        bounds: Rect,
        theme: &Theme,
    );
}
