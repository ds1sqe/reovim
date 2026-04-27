//! Compositor API for window layout operations.
//!
//! This module provides the `CompositorApi` trait, which offers high-level
//! window management operations that delegate to the nested compositor.
//!
//! # Architecture
//!
//! ```text
//! Command (e.g., FocusLeft)
//!     │
//!     ↓ calls
//! CompositorApi.navigate(Left)
//!     │
//!     ↓ delegates to
//! RootCompositor.layer_compositor(active_layer)
//!     │
//!     ↓ then
//! WindowLayerCompositor.navigate_tiled(from, Left)
//! ```
//!
//! # Overlay Operations (#399)
//!
//! Overlays are temporary UI elements (popups, menus, tooltips) that appear
//! above all other windows. They do NOT auto-focus when shown - focus remains
//! on the underlying tiled/float window.
//!
use {
    reovim_kernel::api::v1::TabId,
    reovim_subsys_layout::{
        Direction, LayerId, OverlayConstraints, Rect, SplitDirection, WindowId, WindowPlacement,
    },
};

/// Errors from compositor operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompositorError {
    /// Cannot close the last window.
    CannotCloseLastWindow,
    /// No window in the specified direction.
    NoNeighbor(Direction),
    /// Not enough room to split.
    NotEnoughRoom,
    /// Cannot resize at screen edge.
    CannotResizeAtEdge,
    /// No active layer to operate on.
    NoActiveLayer,
    /// No focused window in the active layer.
    NoFocusedWindow,
    /// Window not found.
    WindowNotFound(WindowId),
    /// Layer not found.
    LayerNotFound(LayerId),
    /// No tab pages available.
    NoTabPages,
    /// Cannot close the last tab page.
    CannotCloseLastTab,
    /// Tab page not found.
    TabNotFound(TabId),
}

impl std::fmt::Display for CompositorError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::CannotCloseLastWindow => write!(f, "cannot close last window"),
            Self::NoNeighbor(dir) => write!(f, "no window {dir:?}"),
            Self::NotEnoughRoom => write!(f, "not enough room to split"),
            Self::CannotResizeAtEdge => write!(f, "cannot resize at edge"),
            Self::NoActiveLayer => write!(f, "no active layer"),
            Self::NoFocusedWindow => write!(f, "no focused window"),
            Self::WindowNotFound(id) => write!(f, "window {} not found", id.as_usize()),
            Self::LayerNotFound(id) => write!(f, "layer {} not found", id.as_u16()),
            Self::NoTabPages => write!(f, "no tab pages"),
            Self::CannotCloseLastTab => write!(f, "cannot close last tab"),
            Self::TabNotFound(id) => write!(f, "tab {} not found", id.as_usize()),
        }
    }
}

impl std::error::Error for CompositorError {}

/// Compositor operations for window management.
///
/// This trait provides high-level operations that commands use to manage
/// windows. It abstracts over the compositor implementation, allowing
/// commands to work with any compositor.
///
/// # Design
///
/// Operations work with the **currently focused window** in the **active layer**.
/// This matches vim's window model where `<C-w>` commands operate on the
/// current window without requiring explicit window arguments.
///
/// # Examples
///
/// ```ignore
/// // In a command handler:
/// fn execute(&self, runtime: &mut SessionRuntime<'_>, _ctx: &CommandContext) -> CommandResult {
///     match runtime.navigate(Direction::Left) {
///         Ok(window) => {
///             runtime.focus(window)?;
///             CommandResult::Success
///         }
///         Err(e) => CommandResult::Error(e.to_string()),
///     }
/// }
/// ```
pub trait CompositorApi {
    /// Navigate in direction from the current window.
    ///
    /// Returns the window ID of the neighbor in that direction, or an error
    /// if there is no neighbor (at screen edge).
    ///
    /// # Errors
    ///
    /// - `NoActiveLayer` - No layer is active
    /// - `NoFocusedWindow` - No window is focused
    /// - `NoNeighbor` - No window in the specified direction
    fn navigate(&self, direction: Direction) -> Result<WindowId, CompositorError>;

    /// Split the current window.
    ///
    /// Creates a new window adjacent to the current window in the specified
    /// direction. The new window is focused after creation.
    ///
    /// # Errors
    ///
    /// - `NoActiveLayer` - No layer is active
    /// - `NoFocusedWindow` - No window is focused
    /// - `NotEnoughRoom` - Window is too small to split
    fn split(&mut self, direction: SplitDirection) -> Result<WindowId, CompositorError>;

    /// Close the current window.
    ///
    /// Returns the window that will receive focus after closing.
    /// Named `close_current_window` to avoid conflict with `WindowApi::close_window`.
    ///
    /// # Errors
    ///
    /// - `CannotCloseLastWindow` - This is the only window
    /// - `NoActiveLayer` - No layer is active
    /// - `NoFocusedWindow` - No window is focused
    fn close_current_window(&mut self) -> Result<WindowId, CompositorError>;

    /// Close all windows except the current one.
    ///
    /// # Errors
    ///
    /// - `NoActiveLayer` - No layer is active
    /// - `NoFocusedWindow` - No window is focused
    fn close_others(&mut self) -> Result<(), CompositorError>;

    /// Resize the current window.
    ///
    /// Moves the edge of the current window in the specified direction.
    /// Positive delta expands in that direction, negative contracts.
    ///
    /// # Arguments
    ///
    /// * `direction` - Which edge to move (Left/Right/Up/Down)
    /// * `delta` - Amount to move (positive = expand, negative = contract)
    ///
    /// # Errors
    ///
    /// - `NoActiveLayer` - No layer is active
    /// - `NoFocusedWindow` - No window is focused
    /// - `CannotResizeAtEdge` - Window is at screen edge in that direction
    fn resize(&mut self, direction: Direction, delta: i16) -> Result<(), CompositorError>;

    /// Equalize all windows in the current layer.
    ///
    /// Distributes space equally among all tiled windows.
    ///
    /// # Errors
    ///
    /// - `NoActiveLayer` - No layer is active
    fn equalize(&mut self) -> Result<(), CompositorError>;

    /// Cycle to next/previous window.
    ///
    /// Windows are ordered by winnr (top-to-bottom, left-to-right).
    ///
    /// # Arguments
    ///
    /// * `forward` - `true` for next window, `false` for previous
    ///
    /// # Errors
    ///
    /// - `NoActiveLayer` - No layer is active
    /// - `NoFocusedWindow` - No window is focused
    fn cycle(&self, forward: bool) -> Result<WindowId, CompositorError>;

    /// Set focus to a specific window.
    ///
    /// Also activates the layer containing the window.
    ///
    /// # Errors
    ///
    /// - `WindowNotFound` - Window does not exist
    fn focus(&mut self, window: WindowId) -> Result<(), CompositorError>;

    /// Get the currently focused window.
    ///
    /// Returns `None` if no window is focused.
    fn focused_window(&self) -> Option<WindowId>;

    /// Get window count in the active layer.
    fn compositor_window_count(&self) -> usize;

    /// Get all window placements (for rendering).
    ///
    /// Returns windows in z-order (lowest first) for proper rendering.
    fn arrange(&self, screen: Rect) -> Vec<WindowPlacement>;

    /// Get the active layer ID.
    fn active_layer(&self) -> Option<LayerId>;

    /// Update screen size (on terminal resize).
    fn set_screen(&mut self, screen: Rect);

    // =========================================================================
    // Float Zone Operations (#398)
    // =========================================================================

    /// Toggle the current window between tiled and float zones.
    ///
    /// If the window is tiled, it becomes a floating window (80% centered).
    /// If the window is floating, it returns to the tiled zone.
    ///
    /// # Errors
    ///
    /// - `NoActiveLayer` - No layer is active
    /// - `NoFocusedWindow` - No window is focused
    fn toggle_float(&mut self) -> Result<(), CompositorError>;

    /// Raise the current float window to the front.
    ///
    /// No-op if the current window is not a float.
    ///
    /// # Errors
    ///
    /// - `NoActiveLayer` - No layer is active
    /// - `NoFocusedWindow` - No window is focused
    fn raise_float(&mut self) -> Result<(), CompositorError>;

    /// Lower the current float window to the back.
    ///
    /// No-op if the current window is not a float.
    ///
    /// # Errors
    ///
    /// - `NoActiveLayer` - No layer is active
    /// - `NoFocusedWindow` - No window is focused
    fn lower_float(&mut self) -> Result<(), CompositorError>;

    // =========================================================================
    // Opacity Operations (#400)
    // =========================================================================

    /// Set the opacity of the active layer.
    ///
    /// # Arguments
    ///
    /// * `opacity` - Value clamped to 0.0..=1.0
    ///
    /// # Errors
    ///
    /// - `NoActiveLayer` - No layer is active
    fn set_active_layer_opacity(&mut self, opacity: f32) -> Result<(), CompositorError>;

    /// Get the opacity of the active layer.
    ///
    /// # Errors
    ///
    /// - `NoActiveLayer` - No layer is active
    fn active_layer_opacity(&self) -> Result<f32, CompositorError>;

    /// Adjust the active layer's opacity by a delta.
    ///
    /// The result is clamped to 0.0..=1.0.
    ///
    /// # Arguments
    ///
    /// * `delta` - Amount to add (positive = more opaque, negative = more transparent)
    ///
    /// # Returns
    ///
    /// The new opacity value after adjustment.
    ///
    /// # Errors
    ///
    /// - `NoActiveLayer` - No layer is active
    fn adjust_active_layer_opacity(&mut self, delta: f32) -> Result<f32, CompositorError>;

    // =========================================================================
    // Overlay Zone Operations (#399)
    // =========================================================================

    /// Show an overlay with constraints.
    ///
    /// Creates a new overlay positioned according to the constraints.
    /// Overlays do NOT auto-focus - they are temporary UI elements that
    /// appear above content without stealing keyboard input.
    ///
    /// # Arguments
    ///
    /// * `constraints` - Positioning constraints (anchor, preferred size, max size)
    ///
    /// # Returns
    ///
    /// The window ID of the new overlay.
    ///
    /// # Errors
    ///
    /// - `NoActiveLayer` - No layer is active
    fn show_overlay(
        &mut self,
        constraints: OverlayConstraints,
    ) -> Result<WindowId, CompositorError>;

    /// Hide (remove) an overlay.
    ///
    /// # Arguments
    ///
    /// * `window` - The overlay window ID to hide
    ///
    /// # Errors
    ///
    /// - `NoActiveLayer` - No layer is active
    fn hide_overlay(&mut self, window: WindowId) -> Result<(), CompositorError>;

    /// Resize an overlay.
    ///
    /// Updates the preferred size of the overlay.
    ///
    /// # Errors
    ///
    /// - `NoActiveLayer` - No layer is active
    fn resize_overlay(
        &mut self,
        window: WindowId,
        width: u16,
        height: u16,
    ) -> Result<(), CompositorError>;

    /// Hide all overlays in the active layer.
    ///
    /// Useful for commands like "dismiss all popups" (Escape key behavior).
    ///
    /// # Errors
    ///
    /// - `NoActiveLayer` - No layer is active
    fn hide_all_overlays(&mut self) -> Result<(), CompositorError>;

    // =========================================================================
    // Tab Page Operations (#401)
    // =========================================================================

    /// Create a new tab page after the active tab.
    ///
    /// The new tab becomes active.
    ///
    /// # Errors
    ///
    /// - `NoTabPages` - Tab system not initialized
    fn tab_new(&mut self) -> Result<TabId, CompositorError>;

    /// Close the active tab page.
    ///
    /// # Errors
    ///
    /// - `NoTabPages` - Tab system not initialized
    /// - `CannotCloseLastTab` - Cannot close the only tab
    fn tab_close(&mut self) -> Result<(), CompositorError>;

    /// Switch to the next tab page (wraps around).
    ///
    /// # Errors
    ///
    /// - `NoTabPages` - Tab system not initialized
    fn tab_next(&mut self) -> Result<TabId, CompositorError>;

    /// Switch to the previous tab page (wraps around).
    ///
    /// # Errors
    ///
    /// - `NoTabPages` - Tab system not initialized
    fn tab_prev(&mut self) -> Result<TabId, CompositorError>;

    /// Switch to a tab page by 0-based index.
    ///
    /// # Errors
    ///
    /// - `NoTabPages` - Tab system not initialized
    /// - `TabNotFound` - Index out of bounds
    fn tab_goto(&mut self, index: usize) -> Result<TabId, CompositorError>;

    /// Get the number of tab pages.
    fn tab_count(&self) -> usize;

    /// Get the active tab page's ID.
    fn active_tab_id(&self) -> Option<TabId>;
}
#[cfg(test)]
#[path = "tests/compositor.rs"]
mod tests;
