//! Overlay registry for plugin-based overlay management
//!
//! This module provides:
//! - `OverlayRenderer` trait for overlays that render to `FrameBuffer`
//! - `OverlayRegistry` for storing and managing registered overlays
//!
//! Plugins register overlays via `PluginContext::register_overlay()`.
//! The runtime iterates over the registry during rendering.

use std::{
    collections::BTreeMap,
    sync::{Arc, RwLock},
};

use crate::{component::RenderContext, frame::FrameBuffer, overlay::OverlayBounds};

/// Trait for overlays that render to a frame buffer
///
/// This is the primary interface for plugin-based overlays.
/// Overlays are stateless structs that access their state through
/// `RenderContext`, which provides access to `PluginStateRegistry`
/// and other runtime state.
///
/// # Example
///
/// ```ignore
/// struct MyOverlay;
///
/// impl OverlayRenderer for MyOverlay {
///     fn id(&self) -> &'static str { "my_overlay" }
///     fn z_order(&self) -> u16 { 150 }
///
///     fn is_visible(&self, ctx: &RenderContext<'_>) -> bool {
///         ctx.state()
///             .and_then(|s| s.plugin_state.with::<MyState, _, _>(|st| st.is_active()))
///             .unwrap_or(false)
///     }
///
///     fn render(&self, buffer: &mut FrameBuffer, ctx: &RenderContext<'_>) {
///         if let Some(state) = ctx.state() {
///             state.plugin_state.with::<MyState, _, _>(|st| {
///                 // Render using st
///             });
///         }
///     }
///
///     fn bounds(&self, ctx: &RenderContext<'_>) -> OverlayBounds {
///         OverlayBounds::new(0, 0, 20, 10)
///     }
/// }
/// ```
pub trait OverlayRenderer: Send + Sync {
    /// Unique identifier for this overlay type
    fn id(&self) -> &'static str;

    /// Z-order for rendering (higher = on top)
    ///
    /// Standard z-orders:
    /// - 100: Leap labels
    /// - 200: Completion popup
    /// - 300: Telescope
    /// - 400: Settings menu
    fn z_order(&self) -> u16;

    /// Whether this overlay is currently visible
    ///
    /// Access state through `ctx.state()` to determine visibility.
    fn is_visible(&self, ctx: &RenderContext<'_>) -> bool;

    /// Render the overlay to the frame buffer
    ///
    /// # Arguments
    /// - `buffer`: Target frame buffer
    /// - `ctx`: Render context with screen dimensions, theme, and state access
    fn render(&self, buffer: &mut FrameBuffer, ctx: &RenderContext<'_>);

    /// Compute bounds for this overlay
    ///
    /// Used for hit testing and layout calculations.
    /// Access screen dimensions via `ctx.screen_width` and `ctx.screen_height`.
    fn bounds(&self, ctx: &RenderContext<'_>) -> OverlayBounds;

    /// Whether this overlay captures input when visible
    ///
    /// When true, the overlay consumes input events.
    /// Default: true
    fn captures_input(&self, _ctx: &RenderContext<'_>) -> bool {
        true
    }

    /// Get cursor position if this overlay owns the cursor
    ///
    /// Returns `Some((x, y))` if the overlay wants to position the cursor,
    /// `None` if the overlay doesn't control the cursor.
    fn cursor_position(&self, _ctx: &RenderContext<'_>) -> Option<(u16, u16)> {
        None
    }
}

/// Entry for a registered overlay
struct OverlayEntry {
    /// The overlay renderer
    overlay: Arc<RwLock<dyn OverlayRenderer>>,
    /// Cached z-order for sorting
    z_order: u16,
}

/// Registry for managing overlays
///
/// Overlays are stored in order of registration and sorted by z-order
/// during rendering. The registry is thread-safe and can be accessed
/// from multiple tasks.
pub struct OverlayRegistry {
    /// Registered overlays by ID
    overlays: BTreeMap<&'static str, OverlayEntry>,
}

impl OverlayRegistry {
    /// Create a new empty registry
    #[must_use]
    #[allow(clippy::missing_const_for_fn)] // BTreeMap::new() not const-stable
    pub fn new() -> Self {
        Self {
            overlays: BTreeMap::new(),
        }
    }

    /// Register an overlay
    ///
    /// If an overlay with the same ID already exists, it will be replaced.
    pub fn register<O: OverlayRenderer + 'static>(&mut self, overlay: O) {
        let id = overlay.id();
        let z_order = overlay.z_order();
        self.overlays.insert(
            id,
            OverlayEntry {
                overlay: Arc::new(RwLock::new(overlay)),
                z_order,
            },
        );
    }

    /// Register an overlay from an Arc
    ///
    /// # Panics
    ///
    /// Panics if the overlay's `RwLock` is poisoned.
    pub fn register_arc(&mut self, overlay: Arc<RwLock<dyn OverlayRenderer>>) {
        let guard = overlay.read().expect("overlay lock poisoned");
        let id = guard.id();
        let z_order = guard.z_order();
        drop(guard);

        self.overlays.insert(id, OverlayEntry { overlay, z_order });
    }

    /// Get an overlay by ID
    #[must_use]
    pub fn get(&self, id: &str) -> Option<Arc<RwLock<dyn OverlayRenderer>>> {
        self.overlays.get(id).map(|e| Arc::clone(&e.overlay))
    }

    /// Remove an overlay by ID
    pub fn remove(&mut self, id: &str) -> Option<Arc<RwLock<dyn OverlayRenderer>>> {
        self.overlays.remove(id).map(|e| e.overlay)
    }

    /// Check if an overlay is registered
    #[must_use]
    pub fn contains(&self, id: &str) -> bool {
        self.overlays.contains_key(id)
    }

    /// Get all visible overlays sorted by z-order (ascending)
    ///
    /// Lower z-order overlays are rendered first (below higher ones).
    #[must_use]
    pub fn visible_overlays_sorted(
        &self,
        ctx: &RenderContext<'_>,
    ) -> Vec<Arc<RwLock<dyn OverlayRenderer>>> {
        let mut entries: Vec<_> = self
            .overlays
            .values()
            .filter(|e| e.overlay.read().map(|o| o.is_visible(ctx)).unwrap_or(false))
            .collect();

        entries.sort_by_key(|e| e.z_order);
        entries
            .into_iter()
            .map(|e| Arc::clone(&e.overlay))
            .collect()
    }

    /// Render all visible overlays in z-order
    ///
    /// Overlays are rendered in ascending z-order (lower first).
    ///
    /// Note: This method cannot render directly because overlays are behind
    /// `RwLock`. Use `visible_overlays_sorted()` and render each overlay
    /// individually in the calling code.
    #[allow(dead_code, clippy::missing_const_for_fn)]
    pub fn render_all(&self, _buffer: &mut FrameBuffer, _ctx: &RenderContext<'_>) {
        // NOTE: Direct rendering from here is not possible because we hold
        // immutable references to the overlays. The Screen should use
        // `visible_overlays_sorted()` and render each overlay in a loop.
        //
        // This method is kept as a placeholder for future optimization where
        // we might use interior mutability or a different locking strategy.
    }

    /// Iterate over all overlays (for manual rendering)
    pub fn iter(&self) -> impl Iterator<Item = (&'static str, &Arc<RwLock<dyn OverlayRenderer>>)> {
        self.overlays.iter().map(|(k, v)| (*k, &v.overlay))
    }

    /// Get the topmost overlay that captures input
    #[must_use]
    pub fn topmost_input_captor(
        &self,
        ctx: &RenderContext<'_>,
    ) -> Option<Arc<RwLock<dyn OverlayRenderer>>> {
        self.overlays
            .values()
            .filter(|e| {
                e.overlay
                    .read()
                    .map(|o| o.is_visible(ctx) && o.captures_input(ctx))
                    .unwrap_or(false)
            })
            .max_by_key(|e| e.z_order)
            .map(|e| Arc::clone(&e.overlay))
    }

    /// Get cursor position from topmost overlay that provides one
    #[must_use]
    pub fn cursor_position(&self, ctx: &RenderContext<'_>) -> Option<(u16, u16)> {
        // Check overlays in reverse z-order (highest first)
        let mut entries: Vec<_> = self
            .overlays
            .values()
            .filter(|e| e.overlay.read().map(|o| o.is_visible(ctx)).unwrap_or(false))
            .collect();

        entries.sort_by_key(|e| std::cmp::Reverse(e.z_order));

        for entry in entries {
            if let Ok(o) = entry.overlay.read()
                && let Some(pos) = o.cursor_position(ctx)
            {
                return Some(pos);
            }
        }
        None
    }

    /// Get the number of registered overlays
    #[must_use]
    pub fn len(&self) -> usize {
        self.overlays.len()
    }

    /// Check if the registry is empty
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.overlays.is_empty()
    }
}

impl Default for OverlayRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use {super::*, crate::highlight::Theme};

    // Test overlay implementation
    struct TestOverlay {
        visible: bool,
        z: u16,
    }

    impl OverlayRenderer for TestOverlay {
        fn id(&self) -> &'static str {
            "test"
        }

        fn z_order(&self) -> u16 {
            self.z
        }

        fn is_visible(&self, _ctx: &RenderContext<'_>) -> bool {
            self.visible
        }

        fn render(&self, _buffer: &mut FrameBuffer, _ctx: &RenderContext<'_>) {
            // No-op for test
        }

        fn bounds(&self, _ctx: &RenderContext<'_>) -> OverlayBounds {
            OverlayBounds::new(0, 0, 10, 5)
        }
    }

    fn test_ctx() -> RenderContext<'static> {
        static THEME: std::sync::LazyLock<Theme> = std::sync::LazyLock::new(Theme::default);
        RenderContext::new(80, 24, &THEME, crate::highlight::ColorMode::TrueColor)
    }

    #[test]
    fn test_registry_register() {
        let mut registry = OverlayRegistry::new();
        registry.register(TestOverlay {
            visible: true,
            z: 100,
        });
        assert!(registry.contains("test"));
        assert_eq!(registry.len(), 1);
    }

    #[test]
    #[allow(clippy::items_after_statements)]
    fn test_registry_visible_sorted() {
        let mut registry = OverlayRegistry::new();
        let ctx = test_ctx();

        // Register overlays with different z-orders
        struct HighOverlay;
        impl OverlayRenderer for HighOverlay {
            fn id(&self) -> &'static str {
                "high"
            }
            fn z_order(&self) -> u16 {
                300
            }
            fn is_visible(&self, _ctx: &RenderContext<'_>) -> bool {
                true
            }
            fn render(&self, _: &mut FrameBuffer, _: &RenderContext<'_>) {}
            fn bounds(&self, _ctx: &RenderContext<'_>) -> OverlayBounds {
                OverlayBounds::default()
            }
        }

        struct LowOverlay;
        impl OverlayRenderer for LowOverlay {
            fn id(&self) -> &'static str {
                "low"
            }
            fn z_order(&self) -> u16 {
                100
            }
            fn is_visible(&self, _ctx: &RenderContext<'_>) -> bool {
                true
            }
            fn render(&self, _: &mut FrameBuffer, _: &RenderContext<'_>) {}
            fn bounds(&self, _ctx: &RenderContext<'_>) -> OverlayBounds {
                OverlayBounds::default()
            }
        }

        registry.register(HighOverlay);
        registry.register(LowOverlay);

        let sorted = registry.visible_overlays_sorted(&ctx);
        assert_eq!(sorted.len(), 2);

        // Lower z-order should be first
        assert_eq!(sorted[0].read().unwrap().id(), "low");
        assert_eq!(sorted[1].read().unwrap().id(), "high");
    }
}
