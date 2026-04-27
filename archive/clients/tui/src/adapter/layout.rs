//! Layout trait implementation for TUI.
//!
//! This module provides adapters between the common client model's `Layout` trait
//! and TUI's `RootCompositor`/`WindowLayerCompositor` traits.

use std::sync::{Arc, Mutex, MutexGuard};

use {
    reovim_client_model::{
        Direction as ModelDirection, LogicalLayout, SplitDirection as ModelSplitDirection,
        traits::Layout,
    },
    reovim_driver_display::{
        WindowId,
        layout::{
            Direction as LayoutDirection, LayerId, RootCompositor,
            SplitDirection as LayoutSplitDirection,
        },
    },
};

/// Adapter implementing the common `Layout` trait for TUI.
///
/// Wraps a `RootCompositor` and delegates layout operations to it.
/// Uses `Arc<Mutex<_>>` to allow shared ownership in async contexts.
///
/// # ID Conversion
///
/// The common model uses `u64` for viewport IDs while TUI uses `WindowId` (usize).
/// This adapter handles the conversion transparently.
pub struct TuiLayoutAdapter<C: RootCompositor + 'static> {
    /// The underlying root compositor.
    compositor: Arc<Mutex<C>>,
    /// The active layer for tiled operations.
    active_layer: LayerId,
}

impl<C: RootCompositor + 'static> TuiLayoutAdapter<C> {
    /// Create a new layout adapter.
    ///
    /// # Arguments
    ///
    /// * `compositor` - The root compositor to wrap
    /// * `active_layer` - The layer ID for tiled window operations
    #[must_use]
    pub const fn new(compositor: Arc<Mutex<C>>, active_layer: LayerId) -> Self {
        Self {
            compositor,
            active_layer,
        }
    }

    /// Get a reference to the underlying compositor.
    #[must_use]
    pub fn compositor(&self) -> Arc<Mutex<C>> {
        Arc::clone(&self.compositor)
    }

    /// Get the active layer ID.
    #[must_use]
    pub const fn active_layer(&self) -> LayerId {
        self.active_layer
    }

    /// Set the active layer for tiled operations.
    pub const fn set_active_layer(&mut self, layer: LayerId) {
        self.active_layer = layer;
    }

    /// Convert a `WindowId` (usize) to `u64`.
    #[must_use]
    const fn window_id_to_u64(id: WindowId) -> u64 {
        id.as_usize() as u64
    }

    /// Convert a `u64` to `WindowId`.
    #[must_use]
    const fn u64_to_window_id(id: u64) -> WindowId {
        #[allow(clippy::cast_possible_truncation)]
        WindowId::from_raw(id as usize)
    }

    /// Lock the compositor mutex.
    ///
    /// # Panics
    ///
    /// Panics if the mutex is poisoned.
    fn lock(&self) -> MutexGuard<'_, C> {
        self.compositor.lock().expect("compositor mutex poisoned")
    }

    /// Convert common model `Direction` to layout `Direction`.
    const fn to_direction(direction: ModelDirection) -> LayoutDirection {
        match direction {
            ModelDirection::Up => LayoutDirection::Up,
            ModelDirection::Down => LayoutDirection::Down,
            ModelDirection::Left => LayoutDirection::Left,
            ModelDirection::Right => LayoutDirection::Right,
        }
    }

    /// Convert common model `SplitDirection` to layout `SplitDirection`.
    const fn to_split_direction(direction: ModelSplitDirection) -> LayoutSplitDirection {
        match direction {
            ModelSplitDirection::Horizontal => LayoutSplitDirection::Horizontal,
            ModelSplitDirection::Vertical => LayoutSplitDirection::Vertical,
        }
    }
}

#[allow(clippy::significant_drop_tightening)]
#[cfg_attr(coverage_nightly, coverage(off))]
impl<C: RootCompositor + 'static> Layout for TuiLayoutAdapter<C> {
    fn split(&mut self, direction: ModelSplitDirection) -> u64 {
        let mut compositor = self.lock();

        // Get the layer compositor for the active layer
        if let Some(layer) = compositor.layer_compositor_mut(self.active_layer) {
            // Get the currently focused window
            if let Some(focused) = layer.focused() {
                // Split in the specified direction
                let split_dir = Self::to_split_direction(direction);
                if let Some(new_id) = layer.split_tiled(focused, split_dir) {
                    return Self::window_id_to_u64(new_id);
                }
            }
        }

        // Return 0 to indicate failure (no new window created)
        0
    }

    fn close(&mut self, viewport_id: u64) -> bool {
        let window_id = Self::u64_to_window_id(viewport_id);
        let mut compositor = self.lock();

        compositor
            .layer_compositor_mut(self.active_layer)
            .is_some_and(|layer| layer.close_tiled(window_id).is_some())
    }

    fn focus(&mut self, viewport_id: u64) -> bool {
        let window_id = Self::u64_to_window_id(viewport_id);
        let mut compositor = self.lock();
        compositor.set_focus(window_id);
        true
    }

    fn focus_direction(&mut self, direction: ModelDirection) -> bool {
        let direction = Self::to_direction(direction);
        let mut compositor = self.lock();

        // Find the target window first (immutable borrow scope)
        let target = compositor
            .layer_compositor(self.active_layer)
            .and_then(|layer| {
                let focused = layer.focused()?;
                layer.navigate_tiled(focused, direction)
            });

        // Set focus if target found (mutable operation after immutable scope ends)
        if let Some(target_window) = target {
            compositor.set_focus(target_window);
            return true;
        }
        false
    }

    fn focused_viewport(&self) -> u64 {
        let compositor = self.lock();
        compositor.focused().map_or(0, Self::window_id_to_u64)
    }

    fn to_logical(&self) -> LogicalLayout {
        let compositor = self.lock();

        // For now, create a simplified logical layout
        // A full implementation would traverse the compositor's window tree
        compositor.focused().map_or_else(
            || LogicalLayout::single(0, 0),
            |focused| {
                let id = Self::window_id_to_u64(focused);
                LogicalLayout::single(id, id)
            },
        )
    }

    fn apply_layout(&mut self, _layout: &LogicalLayout) {
        // Full implementation would reconstruct window arrangement
        // This requires creating/closing windows to match the logical layout
        // For now, this is a no-op - future work will implement this
    }

    fn window_count(&self) -> usize {
        let compositor = self.lock();
        compositor.window_count()
    }
}

#[cfg(test)]
#[path = "layout_tests.rs"]
mod tests;
