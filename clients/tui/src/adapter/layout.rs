//! Layout trait implementation for TUI.
//!
//! This module provides adapters between the common client model's `Layout` trait
//! and TUI's `RootCompositor`/`WindowLayerCompositor` traits.

use std::sync::{Arc, Mutex, MutexGuard};

use {
    reovim_client_model::{Direction, LogicalLayout, SplitDirection, traits::Layout},
    reovim_driver_display::{
        NavigateDirection, WindowId,
        layout::{LayerId, RootCompositor},
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

    /// Convert common model Direction to TUI `NavigateDirection`.
    const fn to_navigate_direction(direction: Direction) -> NavigateDirection {
        match direction {
            Direction::Up => NavigateDirection::Up,
            Direction::Down => NavigateDirection::Down,
            Direction::Left => NavigateDirection::Left,
            Direction::Right => NavigateDirection::Right,
        }
    }

    /// Convert common model `SplitDirection` to TUI `SplitDirection`.
    const fn to_split_direction(
        direction: SplitDirection,
    ) -> reovim_driver_display::SplitDirection {
        match direction {
            SplitDirection::Horizontal => reovim_driver_display::SplitDirection::Horizontal,
            SplitDirection::Vertical => reovim_driver_display::SplitDirection::Vertical,
        }
    }
}

#[allow(clippy::significant_drop_tightening)]
impl<C: RootCompositor + 'static> Layout for TuiLayoutAdapter<C> {
    fn split(&mut self, direction: SplitDirection) -> u64 {
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

    fn focus_direction(&mut self, direction: Direction) -> bool {
        let nav_dir = Self::to_navigate_direction(direction);
        let mut compositor = self.lock();

        // Find the target window first (immutable borrow scope)
        let target = compositor
            .layer_compositor(self.active_layer)
            .and_then(|layer| {
                let focused = layer.focused()?;
                layer.navigate_tiled(focused, nav_dir)
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
mod tests {
    use {super::*, reovim_driver_display::layout::WindowLayerCompositor};

    // Mock compositor for testing
    struct MockCompositor {
        windows: Vec<WindowId>,
        focused: Option<WindowId>,
        next_id: usize,
    }

    impl MockCompositor {
        fn new() -> Self {
            let first = WindowId::from_raw(1);
            Self {
                windows: vec![first],
                focused: Some(first),
                next_id: 2,
            }
        }
    }

    // Minimal RootCompositor implementation for testing
    impl RootCompositor for MockCompositor {
        fn composite(
            &self,
            screen: reovim_driver_display::Rect,
        ) -> reovim_driver_display::layout::CompositeResult {
            reovim_driver_display::layout::CompositeResult::empty(screen)
        }

        fn create_layer(&mut self, _config: reovim_driver_display::layout::LayerConfig) -> LayerId {
            LayerId::new(0)
        }

        fn remove_layer(&mut self, _layer: LayerId) {}

        fn layer_by_label(&self, _label: &str) -> Option<LayerId> {
            Some(LayerId::new(0))
        }

        fn layers(&self) -> Vec<&reovim_driver_display::layout::Layer> {
            Vec::new()
        }

        fn set_layer_visible(&mut self, _layer: LayerId, _visible: bool) {}

        fn set_layer_opacity(&mut self, _layer: LayerId, _opacity: f32) {}

        fn reorder_layer(&mut self, _layer: LayerId, _new_z: u16) {}

        fn set_active_layer(&mut self, _layer: LayerId) {}

        fn active_layer(&self) -> Option<LayerId> {
            Some(LayerId::new(0))
        }

        fn set_focus(&mut self, window: WindowId) {
            if self.windows.contains(&window) {
                self.focused = Some(window);
            }
        }

        fn focused(&self) -> Option<WindowId> {
            self.focused
        }

        fn focus_at(&mut self, _x: u16, _y: u16) -> Option<WindowId> {
            self.focused
        }

        fn layer_compositor(&self, _layer: LayerId) -> Option<&dyn WindowLayerCompositor> {
            None
        }

        fn layer_compositor_mut(
            &mut self,
            _layer: LayerId,
        ) -> Option<&mut dyn WindowLayerCompositor> {
            None
        }

        fn window_count(&self) -> usize {
            self.windows.len()
        }

        fn set_screen(&mut self, _screen: reovim_driver_display::Rect) {}

        fn layer_of(&self, _window: WindowId) -> Option<LayerId> {
            Some(LayerId::new(0))
        }

        fn boxed_clone(&self) -> Box<dyn RootCompositor> {
            Box::new(Self {
                windows: self.windows.clone(),
                focused: self.focused,
                next_id: self.next_id,
            })
        }
    }

    #[test]
    fn test_layout_adapter_new() {
        let compositor = Arc::new(Mutex::new(MockCompositor::new()));
        let adapter = TuiLayoutAdapter::new(compositor, LayerId::new(0));
        assert_eq!(adapter.active_layer(), LayerId::new(0));
    }

    #[test]
    fn test_layout_adapter_focused_viewport() {
        let compositor = Arc::new(Mutex::new(MockCompositor::new()));
        let adapter = TuiLayoutAdapter::new(compositor, LayerId::new(0));
        assert_eq!(adapter.focused_viewport(), 1); // First window ID
    }

    #[test]
    fn test_layout_adapter_window_count() {
        let compositor = Arc::new(Mutex::new(MockCompositor::new()));
        let adapter = TuiLayoutAdapter::new(compositor, LayerId::new(0));
        assert_eq!(adapter.window_count(), 1);
    }

    #[test]
    fn test_layout_adapter_is_single() {
        let compositor = Arc::new(Mutex::new(MockCompositor::new()));
        let adapter = TuiLayoutAdapter::new(compositor, LayerId::new(0));
        assert!(adapter.is_single());
    }

    #[test]
    fn test_layout_adapter_to_logical() {
        let compositor = Arc::new(Mutex::new(MockCompositor::new()));
        let adapter = TuiLayoutAdapter::new(compositor, LayerId::new(0));
        let logical = adapter.to_logical();
        assert!(logical.is_leaf());
    }

    #[test]
    fn test_layout_adapter_focus() {
        let compositor = Arc::new(Mutex::new(MockCompositor::new()));
        let mut adapter = TuiLayoutAdapter::new(compositor.clone(), LayerId::new(0));

        // Add another window to the mock compositor
        {
            let mut comp = compositor.lock().unwrap();
            let new_id = WindowId::from_raw(2);
            comp.windows.push(new_id);
        }

        // Focus should succeed
        assert!(adapter.focus(2));
        assert_eq!(adapter.focused_viewport(), 2);
    }

    #[test]
    fn test_window_id_conversion() {
        let id = WindowId::from_raw(42);
        let u64_id = TuiLayoutAdapter::<MockCompositor>::window_id_to_u64(id);
        assert_eq!(u64_id, 42);

        let back = TuiLayoutAdapter::<MockCompositor>::u64_to_window_id(u64_id);
        assert_eq!(back, id);
    }

    #[test]
    fn test_direction_conversion() {
        assert!(matches!(
            TuiLayoutAdapter::<MockCompositor>::to_navigate_direction(Direction::Up),
            NavigateDirection::Up
        ));
        assert!(matches!(
            TuiLayoutAdapter::<MockCompositor>::to_navigate_direction(Direction::Down),
            NavigateDirection::Down
        ));
        assert!(matches!(
            TuiLayoutAdapter::<MockCompositor>::to_navigate_direction(Direction::Left),
            NavigateDirection::Left
        ));
        assert!(matches!(
            TuiLayoutAdapter::<MockCompositor>::to_navigate_direction(Direction::Right),
            NavigateDirection::Right
        ));
    }

    #[test]
    fn test_split_direction_conversion() {
        assert!(matches!(
            TuiLayoutAdapter::<MockCompositor>::to_split_direction(SplitDirection::Horizontal),
            reovim_driver_display::SplitDirection::Horizontal
        ));
        assert!(matches!(
            TuiLayoutAdapter::<MockCompositor>::to_split_direction(SplitDirection::Vertical),
            reovim_driver_display::SplitDirection::Vertical
        ));
    }
}
