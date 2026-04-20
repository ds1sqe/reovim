//! Policy traits for layout and focus management.
//!
//! This module defines **contracts** (traits) that modules can implement to provide
//! custom layout and focus behavior. The display driver provides default implementations.
//!
//! # Mechanism vs Policy Separation
//!
//! Following the Linux kernel design principle:
//! - **Mechanism** (this driver): Provides `WindowView`, renders windows at given positions
//! - **Policy** (layout module): Decides WHERE windows go and HOW focus moves
//!
//! # Example
//!
//! ```ignore
//! use reovim_driver_display::{LayoutPolicy, WindowView, WindowId, Rect};
//!
//! struct TilingLayout;
//!
//! impl LayoutPolicy for TilingLayout {
//!     fn arrange(&self, screen_size: (u16, u16), windows: &[WindowId]) -> Vec<WindowView> {
//!         // Tile windows horizontally
//!         let (w, h) = screen_size;
//!         let per_window = w / windows.len().max(1) as u16;
//!         windows.iter().enumerate().map(|(i, &id)| WindowView {
//!             window_id: id,
//!             bounds: Rect::new(i as u16 * per_window, 0, per_window, h),
//!         }).collect()
//!     }
//! }
//! ```

use crate::{
    WindowId,
    window::{Direction, Rect},
};

/// A window with its screen position (from `LayoutPolicy`).
///
/// `WindowView` represents a "placed" window - a window that has been assigned
/// a position on screen by a layout policy. The kernel's Window type has no
/// position; position is determined by the display layer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowView {
    /// The window identifier.
    pub window_id: WindowId,
    /// Screen bounds (position and size).
    pub bounds: Rect,
}

impl WindowView {
    /// Create a new window view.
    #[must_use]
    pub const fn new(window_id: WindowId, bounds: Rect) -> Self {
        Self { window_id, bounds }
    }

    /// Check if a point is within this window's bounds.
    #[must_use]
    pub const fn contains(&self, x: u16, y: u16) -> bool {
        self.bounds.contains_xy(x, y)
    }
}

/// Layout policy decides WHERE windows go on screen.
///
/// This trait is the contract between the display driver (mechanism) and
/// layout modules (policy). The display driver calls `arrange()` to get
/// window positions, then renders windows at those positions.
///
/// # Implementors
///
/// - `SingleWindowLayout` (default): Single window fills screen
/// - `TilingLayout` (in layout module): Tile windows in splits
/// - Custom layouts: Floating, tabbed, etc.
pub trait LayoutPolicy: Send + Sync {
    /// Arrange windows on screen, returning positioned views.
    ///
    /// # Arguments
    ///
    /// * `screen_size` - The screen dimensions (width, height) in columns/rows
    /// * `windows` - The window IDs to arrange
    ///
    /// # Returns
    ///
    /// A vector of `WindowView` with each window's position. Windows not in
    /// the returned vector are considered hidden.
    fn arrange(&self, screen_size: (u16, u16), windows: &[WindowId]) -> Vec<WindowView>;
}

/// Focus policy decides HOW focus moves between windows.
///
/// This trait is the contract for directional navigation. When the user
/// presses Ctrl+W h/j/k/l, the focus policy determines which window
/// receives focus.
///
/// # Implementors
///
/// - `DefaultFocusPolicy` (default): Simple directional navigation
/// - `VimFocusPolicy` (in layout module): Vim-style navigation
pub trait FocusPolicy: Send + Sync {
    /// Get the next window in the given direction from current.
    ///
    /// # Arguments
    ///
    /// * `direction` - The navigation direction (Left, Down, Up, Right)
    /// * `current` - The currently focused window ID
    /// * `views` - All window views (from `LayoutPolicy::arrange`)
    ///
    /// # Returns
    ///
    /// The window ID to focus, or `None` if there's no window in that direction.
    fn next(
        &self,
        direction: Direction,
        current: WindowId,
        views: &[WindowView],
    ) -> Option<WindowId>;
}

/// Default layout: single window fills the entire screen.
///
/// This is the simplest layout - the first window takes the full screen,
/// additional windows are hidden.
#[derive(Debug, Clone, Copy, Default)]
pub struct SingleWindowLayout;

impl LayoutPolicy for SingleWindowLayout {
    fn arrange(&self, (w, h): (u16, u16), windows: &[WindowId]) -> Vec<WindowView> {
        windows
            .first()
            .map(|&id| {
                vec![WindowView {
                    window_id: id,
                    bounds: Rect::new(0, 0, w, h),
                }]
            })
            .unwrap_or_default()
    }
}

/// Default focus policy: simple directional navigation based on window centers.
///
/// Finds the closest window in the given direction based on the center
/// point of each window.
#[derive(Debug, Clone, Copy, Default)]
pub struct DefaultFocusPolicy;

impl FocusPolicy for DefaultFocusPolicy {
    fn next(
        &self,
        direction: Direction,
        current: WindowId,
        views: &[WindowView],
    ) -> Option<WindowId> {
        let current_view = views.iter().find(|v| v.window_id == current)?;
        let (cx, cy) = center(current_view.bounds);

        let mut best: Option<(WindowId, i32)> = None;

        for view in views {
            if view.window_id == current {
                continue;
            }

            let (vx, vy) = center(view.bounds);
            let (dx, dy) = (i32::from(vx) - i32::from(cx), i32::from(vy) - i32::from(cy));

            // Check if window is in the correct direction
            let in_direction = match direction {
                Direction::Left => dx < 0,
                Direction::Right => dx > 0,
                Direction::Up => dy < 0,
                Direction::Down => dy > 0,
            };

            if !in_direction {
                continue;
            }

            // Calculate distance (Manhattan for simplicity)
            let dist = dx.abs() + dy.abs();

            if best.is_none_or(|(_, d)| dist < d) {
                best = Some((view.window_id, dist));
            }
        }

        best.map(|(id, _)| id)
    }
}

/// Get the center point of a rectangle.
const fn center(rect: Rect) -> (u16, u16) {
    (rect.x + rect.width / 2, rect.y + rect.height / 2)
}

#[cfg(test)]
#[path = "policy_tests.rs"]
mod tests;
