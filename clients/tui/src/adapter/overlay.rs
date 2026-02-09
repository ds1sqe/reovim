//! Overlay trait implementations for TUI.
//!
//! This module provides adapters between the common client model's `OverlayManager`
//! and `OverlayRenderer` traits and TUI's `OverlayLayer` infrastructure.

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use {
    reovim_client_model::{
        ScreenPosition, Size,
        interaction::{Interaction, InteractionResult},
        rendered::RenderedOverlay,
        traits::{OverlayManager, OverlayRenderer},
        wire::LogicalOverlay,
    },
    reovim_driver_display::{
        WindowId,
        layout::{Anchor as TuiAnchor, OverlayConstraints, OverlayLayer},
    },
};

use super::anchor::{AnchorContext, convert_anchor};

/// Adapter implementing the common `OverlayManager` trait for TUI.
///
/// Coordinates multiple overlays, handling z-ordering and input routing.
/// Wraps an `OverlayLayer` and tracks the mapping between logical overlay
/// IDs (strings) and TUI `WindowId`s.
pub struct TuiOverlayManager {
    /// Registered renderers for different overlay kinds.
    renderers: Vec<Box<dyn OverlayRenderer>>,
    /// Active overlays (rendered state).
    active: Vec<RenderedOverlay>,
    /// Mapping from overlay ID to `WindowId`.
    id_to_window: HashMap<String, WindowId>,
    /// Mapping from `WindowId` to overlay ID (reverse lookup).
    window_to_id: HashMap<WindowId, String>,
    /// Next window ID for new overlays.
    next_window_id: usize,
    /// Context for anchor conversion (needs to be updated by caller).
    anchor_context: Option<AnchorContext>,
    /// Underlying TUI overlay layer (optional - for testing without real layer).
    layer: Option<Arc<Mutex<dyn OverlayLayer>>>,
}

impl TuiOverlayManager {
    /// Create a new overlay manager.
    #[must_use]
    pub fn new() -> Self {
        Self {
            renderers: Vec::new(),
            active: Vec::new(),
            id_to_window: HashMap::new(),
            window_to_id: HashMap::new(),
            next_window_id: 1000, // Start high to avoid conflict with regular window IDs
            anchor_context: None,
            layer: None,
        }
    }

    /// Create an overlay manager with an underlying TUI layer.
    ///
    /// Only used in interactive mode with real terminal layer.
    #[must_use]
    #[cfg_attr(coverage_nightly, coverage(off))]
    pub fn with_layer(layer: Arc<Mutex<dyn OverlayLayer>>) -> Self {
        Self {
            renderers: Vec::new(),
            active: Vec::new(),
            id_to_window: HashMap::new(),
            window_to_id: HashMap::new(),
            next_window_id: 1000,
            anchor_context: None,
            layer: Some(layer),
        }
    }

    /// Set the anchor context for position resolution.
    ///
    /// This must be called before `show()` to provide cursor position,
    /// screen size, and existing overlay windows for anchor resolution.
    pub fn set_anchor_context(&mut self, ctx: AnchorContext) {
        self.anchor_context = Some(ctx);
    }

    /// Get the `WindowId` for an overlay (for rendering).
    #[must_use]
    pub fn window_id(&self, overlay_id: &str) -> Option<WindowId> {
        self.id_to_window.get(overlay_id).copied()
    }

    /// Get the overlay ID for a `WindowId`.
    #[must_use]
    pub fn overlay_id(&self, window_id: WindowId) -> Option<&str> {
        self.window_to_id.get(&window_id).map(String::as_str)
    }

    /// Allocate a new window ID for an overlay.
    const fn allocate_window_id(&mut self) -> WindowId {
        let id = WindowId::from_raw(self.next_window_id);
        self.next_window_id += 1;
        id
    }

    /// Find a renderer that handles the given overlay kind.
    fn find_renderer(&self, kind: &str) -> Option<&dyn OverlayRenderer> {
        self.renderers
            .iter()
            .find(|r| r.handles(kind))
            .map(AsRef::as_ref)
    }

    /// Convert wire anchor to TUI constraints.
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn to_constraints(&self, overlay: &LogicalOverlay, size: Size) -> OverlayConstraints {
        let anchor_ctx = self
            .anchor_context
            .as_ref()
            .expect("anchor context not set");
        let tui_anchor = convert_anchor(&overlay.anchor, anchor_ctx);

        OverlayConstraints {
            anchor: tui_anchor,
            preferred_width: Some(size.width),
            preferred_height: Some(size.height),
            max_width: None,
            max_height: None,
        }
    }

    /// Calculate the screen position for an overlay.
    fn resolve_position(
        &self,
        overlay: &LogicalOverlay,
        screen_size: Size,
        size: Size,
    ) -> ScreenPosition {
        let anchor_ctx = self
            .anchor_context
            .as_ref()
            .expect("anchor context not set");
        let tui_anchor = convert_anchor(&overlay.anchor, anchor_ctx);

        // Convert TUI anchor to screen position
        match tui_anchor {
            #[allow(clippy::cast_possible_truncation)]
            TuiAnchor::Cursor { line, col, .. } => {
                // Position below cursor
                ScreenPosition::new(col.as_usize() as u16, (line.as_usize() + 1) as u16)
            }
            TuiAnchor::Screen { x, y } => ScreenPosition::new(x, y),
            TuiAnchor::Center => {
                // Center on screen
                let x = screen_size.width.saturating_sub(size.width) / 2;
                let y = screen_size.height.saturating_sub(size.height) / 2;
                ScreenPosition::new(x, y)
            }
            TuiAnchor::Below(window_id) => {
                // Try to find the referenced window's position
                // For now, fall back to a default position
                // In a full implementation, we'd look up the window placement
                let _ = window_id;
                ScreenPosition::new(0, 1)
            }
        }
    }
}

impl Default for TuiOverlayManager {
    fn default() -> Self {
        Self::new()
    }
}

#[allow(clippy::significant_drop_tightening, clippy::if_let_mutex)]
#[cfg_attr(coverage_nightly, coverage(off))]
impl OverlayManager for TuiOverlayManager {
    fn register_renderer(&mut self, renderer: Box<dyn OverlayRenderer>) {
        self.renderers.push(renderer);
    }

    fn show(&mut self, overlay: LogicalOverlay, screen_size: Size) {
        // Find renderer index for this overlay kind (to avoid borrowing self)
        let renderer_idx = self.renderers.iter().position(|r| r.handles(&overlay.kind));

        let Some(renderer_idx) = renderer_idx else {
            return; // No renderer for this kind
        };

        // Measure the overlay (borrow renderer temporarily)
        let size = self.renderers[renderer_idx].measure(&overlay, screen_size);

        // Calculate position
        let position = self.resolve_position(&overlay, screen_size, size);

        // Allocate window ID and track mapping
        let window_id = self.allocate_window_id();
        self.id_to_window.insert(overlay.id.clone(), window_id);
        self.window_to_id.insert(window_id, overlay.id.clone());

        // Update anchor context with new overlay window
        if let Some(ref mut ctx) = self.anchor_context {
            ctx.register_overlay(&overlay.id, window_id);
        }

        // Show in underlying layer if present
        if let Some(ref layer) = self.layer {
            let constraints = self.to_constraints(&overlay, size);
            if let Ok(mut layer) = layer.lock() {
                layer.show(window_id, constraints);
            }
        }

        // Create rendered overlay and add to active list
        let is_modal = self.renderers[renderer_idx].is_modal(&overlay);
        let mut rendered = RenderedOverlay::new(overlay, position, size);
        if is_modal {
            rendered = rendered.as_modal();
        }
        self.active.push(rendered);
    }

    fn hide(&mut self, id: &str) -> bool {
        if let Some(window_id) = self.id_to_window.remove(id) {
            self.window_to_id.remove(&window_id);

            // Hide in underlying layer
            if let Some(ref layer) = self.layer
                && let Ok(mut layer) = layer.lock()
            {
                layer.hide(window_id);
            }

            // Remove from active list
            self.active.retain(|o| o.id() != id);
            true
        } else {
            false
        }
    }

    fn active(&self) -> &[RenderedOverlay] {
        &self.active
    }

    fn has_modal(&self) -> bool {
        self.active.iter().any(|o| o.is_modal)
    }

    fn interact(&mut self, interaction: Interaction) -> InteractionResult {
        // Route to topmost overlay that handles it (reverse order = top first)
        for overlay in self.active.iter().rev() {
            if let Some(renderer) = self.find_renderer(overlay.kind()) {
                let result = renderer.interact(&overlay.logical, interaction.clone());
                if result.was_handled() {
                    return result;
                }
            }
        }
        InteractionResult::PassThrough
    }

    fn update(&mut self, id: &str, overlay: LogicalOverlay) {
        if let Some(pos) = self.active.iter().position(|o| o.id() == id) {
            let old = &self.active[pos];
            let size = old.size;
            let position = old.position;
            let is_modal = old.is_modal;

            let mut new_rendered = RenderedOverlay::new(overlay, position, size);
            if is_modal {
                new_rendered = new_rendered.as_modal();
            }
            self.active[pos] = new_rendered;
        }
    }

    fn clear(&mut self) {
        // Hide all overlays in underlying layer
        if let Some(ref layer) = self.layer
            && let Ok(mut layer) = layer.lock()
        {
            layer.hide_all();
        }

        // Clear tracking state
        self.id_to_window.clear();
        self.window_to_id.clear();
        self.active.clear();
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        reovim_client_model::wire::{Anchor, OverlayState},
    };

    struct TestRenderer {
        kind: String,
        modal: bool,
    }

    impl TestRenderer {
        fn new(kind: &str) -> Self {
            Self {
                kind: kind.to_string(),
                modal: false,
            }
        }

        fn modal(kind: &str) -> Self {
            Self {
                kind: kind.to_string(),
                modal: true,
            }
        }
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    impl OverlayRenderer for TestRenderer {
        fn handles(&self, kind: &str) -> bool {
            self.kind == kind
        }

        fn measure(&self, _overlay: &LogicalOverlay, max_size: Size) -> Size {
            Size::new(max_size.width.min(40), max_size.height.min(10))
        }

        fn interact(
            &self,
            overlay: &LogicalOverlay,
            interaction: Interaction,
        ) -> InteractionResult {
            match interaction {
                Interaction::Confirm => {
                    InteractionResult::confirm(serde_json::json!({"selected": true}))
                }
                Interaction::Cancel => InteractionResult::Cancel,
                Interaction::SelectNext => {
                    let mut state = overlay.state.clone();
                    state.selected_index = Some(state.selected_index.unwrap_or(0) + 1);
                    InteractionResult::state_update(state)
                }
                _ => InteractionResult::PassThrough,
            }
        }

        fn is_modal(&self, _overlay: &LogicalOverlay) -> bool {
            self.modal
        }
    }

    fn test_context() -> AnchorContext {
        AnchorContext::new(WindowId::from_raw(1), (10, 5), Size::new(80, 24))
    }

    #[test]
    fn test_overlay_manager_new() {
        let manager = TuiOverlayManager::new();
        assert!(manager.active().is_empty());
        assert!(!manager.has_modal());
    }

    #[test]
    fn test_overlay_manager_register_renderer() {
        let mut manager = TuiOverlayManager::new();
        manager.register_renderer(Box::new(TestRenderer::new("completion")));
        assert_eq!(manager.renderers.len(), 1);
    }

    #[test]
    fn test_overlay_manager_show() {
        let mut manager = TuiOverlayManager::new();
        manager.register_renderer(Box::new(TestRenderer::new("completion")));
        manager.set_anchor_context(test_context());

        let overlay = LogicalOverlay::new("test-1", "completion", Anchor::Cursor);
        manager.show(overlay, Size::new(80, 24));

        assert_eq!(manager.active().len(), 1);
        assert_eq!(manager.active()[0].id(), "test-1");
    }

    #[test]
    fn test_overlay_manager_show_unknown_kind() {
        let mut manager = TuiOverlayManager::new();
        manager.set_anchor_context(test_context());

        // No renderer registered, should be ignored
        let overlay = LogicalOverlay::new("test-1", "unknown", Anchor::Cursor);
        manager.show(overlay, Size::new(80, 24));

        assert!(manager.active().is_empty());
    }

    #[test]
    fn test_overlay_manager_hide() {
        let mut manager = TuiOverlayManager::new();
        manager.register_renderer(Box::new(TestRenderer::new("completion")));
        manager.set_anchor_context(test_context());

        let overlay = LogicalOverlay::new("test-1", "completion", Anchor::Cursor);
        manager.show(overlay, Size::new(80, 24));
        assert_eq!(manager.active().len(), 1);

        assert!(manager.hide("test-1"));
        assert!(manager.active().is_empty());
    }

    #[test]
    fn test_overlay_manager_hide_unknown() {
        let mut manager = TuiOverlayManager::new();
        assert!(!manager.hide("nonexistent"));
    }

    #[test]
    fn test_overlay_manager_has_modal() {
        let mut manager = TuiOverlayManager::new();
        manager.register_renderer(Box::new(TestRenderer::modal("command-palette")));
        manager.set_anchor_context(test_context());

        assert!(!manager.has_modal());

        let overlay = LogicalOverlay::new("modal-1", "command-palette", Anchor::Center);
        manager.show(overlay, Size::new(80, 24));

        assert!(manager.has_modal());
    }

    #[test]
    fn test_overlay_manager_interact_confirm() {
        let mut manager = TuiOverlayManager::new();
        manager.register_renderer(Box::new(TestRenderer::new("completion")));
        manager.set_anchor_context(test_context());

        let overlay = LogicalOverlay::new("test-1", "completion", Anchor::Cursor);
        manager.show(overlay, Size::new(80, 24));

        let result = manager.interact(Interaction::Confirm);
        assert!(result.closes_overlay());
    }

    #[test]
    fn test_overlay_manager_interact_cancel() {
        let mut manager = TuiOverlayManager::new();
        manager.register_renderer(Box::new(TestRenderer::new("completion")));
        manager.set_anchor_context(test_context());

        let overlay = LogicalOverlay::new("test-1", "completion", Anchor::Cursor);
        manager.show(overlay, Size::new(80, 24));

        let result = manager.interact(Interaction::Cancel);
        assert!(matches!(result, InteractionResult::Cancel));
    }

    #[test]
    fn test_overlay_manager_interact_pass_through() {
        let mut manager = TuiOverlayManager::new();
        manager.register_renderer(Box::new(TestRenderer::new("completion")));
        manager.set_anchor_context(test_context());

        let overlay = LogicalOverlay::new("test-1", "completion", Anchor::Cursor);
        manager.show(overlay, Size::new(80, 24));

        let result = manager.interact(Interaction::custom("unknown"));
        assert!(!result.was_handled());
    }

    #[test]
    fn test_overlay_manager_interact_empty() {
        let mut manager = TuiOverlayManager::new();
        let result = manager.interact(Interaction::Confirm);
        assert!(!result.was_handled());
    }

    #[test]
    fn test_overlay_manager_update() {
        let mut manager = TuiOverlayManager::new();
        manager.register_renderer(Box::new(TestRenderer::new("completion")));
        manager.set_anchor_context(test_context());

        let overlay = LogicalOverlay::new("test-1", "completion", Anchor::Cursor)
            .with_state(OverlayState::with_selection(0));
        manager.show(overlay, Size::new(80, 24));

        let updated = LogicalOverlay::new("test-1", "completion", Anchor::Cursor)
            .with_state(OverlayState::with_selection(5));
        manager.update("test-1", updated);

        assert_eq!(manager.active()[0].logical.state.selected_index, Some(5));
    }

    #[test]
    fn test_overlay_manager_clear() {
        let mut manager = TuiOverlayManager::new();
        manager.register_renderer(Box::new(TestRenderer::new("completion")));
        manager.set_anchor_context(test_context());

        manager
            .show(LogicalOverlay::new("test-1", "completion", Anchor::Cursor), Size::new(80, 24));
        manager
            .show(LogicalOverlay::new("test-2", "completion", Anchor::Cursor), Size::new(80, 24));
        assert_eq!(manager.active().len(), 2);

        manager.clear();
        assert!(manager.active().is_empty());
        assert!(manager.id_to_window.is_empty());
    }

    #[test]
    fn test_overlay_manager_window_id_lookup() {
        let mut manager = TuiOverlayManager::new();
        manager.register_renderer(Box::new(TestRenderer::new("completion")));
        manager.set_anchor_context(test_context());

        let overlay = LogicalOverlay::new("test-1", "completion", Anchor::Cursor);
        manager.show(overlay, Size::new(80, 24));

        // Check forward lookup
        let window_id = manager.window_id("test-1");
        assert!(window_id.is_some());

        // Check reverse lookup
        let overlay_id = manager.overlay_id(window_id.unwrap());
        assert_eq!(overlay_id, Some("test-1"));
    }

    #[test]
    fn test_overlay_manager_multiple_overlays() {
        let mut manager = TuiOverlayManager::new();
        manager.register_renderer(Box::new(TestRenderer::new("completion")));
        manager.register_renderer(Box::new(TestRenderer::new("hover")));
        manager.set_anchor_context(test_context());

        manager.show(
            LogicalOverlay::new("completion-1", "completion", Anchor::Cursor),
            Size::new(80, 24),
        );
        manager.show(LogicalOverlay::new("hover-1", "hover", Anchor::Cursor), Size::new(80, 24));

        assert_eq!(manager.active().len(), 2);

        // Hide one
        manager.hide("completion-1");
        assert_eq!(manager.active().len(), 1);
        assert_eq!(manager.active()[0].id(), "hover-1");
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_overlay_manager_interact_topmost() {
        let mut manager = TuiOverlayManager::new();
        manager.register_renderer(Box::new(TestRenderer::new("completion")));
        manager.set_anchor_context(test_context());

        // Add two overlays - second one is "on top" (last in list)
        manager.show(
            LogicalOverlay::new("bottom", "completion", Anchor::Cursor)
                .with_state(OverlayState::with_selection(0)),
            Size::new(80, 24),
        );
        manager.show(
            LogicalOverlay::new("top", "completion", Anchor::Cursor)
                .with_state(OverlayState::with_selection(10)),
            Size::new(80, 24),
        );

        // SelectNext should affect the topmost overlay
        let result = manager.interact(Interaction::SelectNext);
        if let InteractionResult::StateUpdate(state) = result {
            // Should be 11 (10 + 1), not 1 (0 + 1)
            assert_eq!(state.selected_index, Some(11));
        } else {
            panic!("Expected StateUpdate");
        }
    }

    #[test]
    fn test_overlay_position_center() {
        let mut manager = TuiOverlayManager::new();
        manager.register_renderer(Box::new(TestRenderer::new("completion")));
        manager.set_anchor_context(test_context());

        let overlay = LogicalOverlay::new("centered", "completion", Anchor::Center);
        manager.show(overlay, Size::new(80, 24));

        // Overlay should be centered - size is 40x10, screen is 80x24
        // Expected position: (80-40)/2 = 20, (24-10)/2 = 7
        let pos = manager.active()[0].position;
        assert_eq!(pos.x, 20);
        assert_eq!(pos.y, 7);
    }

    #[test]
    fn test_overlay_position_cursor() {
        let mut manager = TuiOverlayManager::new();
        manager.register_renderer(Box::new(TestRenderer::new("completion")));
        // Context has cursor at line 10, col 5
        manager.set_anchor_context(test_context());

        let overlay = LogicalOverlay::new("at-cursor", "completion", Anchor::Cursor);
        manager.show(overlay, Size::new(80, 24));

        // Should be positioned below cursor (line 10 -> y=11, col 5 -> x=5)
        let pos = manager.active()[0].position;
        assert_eq!(pos.x, 5);
        assert_eq!(pos.y, 11);
    }

    #[test]
    fn test_overlay_manager_default() {
        let manager = TuiOverlayManager::default();
        assert!(manager.active().is_empty());
        assert!(!manager.has_modal());
    }

    #[test]
    fn test_overlay_manager_window_id_nonexistent() {
        let manager = TuiOverlayManager::new();
        assert!(manager.window_id("nonexistent").is_none());
    }

    #[test]
    fn test_overlay_manager_overlay_id_nonexistent() {
        let manager = TuiOverlayManager::new();
        assert!(manager.overlay_id(WindowId::from_raw(999)).is_none());
    }

    #[test]
    fn test_overlay_manager_update_nonexistent() {
        let mut manager = TuiOverlayManager::new();
        manager.register_renderer(Box::new(TestRenderer::new("completion")));
        manager.set_anchor_context(test_context());

        // Update a non-existent overlay - should be a no-op (no panic)
        let updated = LogicalOverlay::new("nonexistent", "completion", Anchor::Cursor);
        manager.update("nonexistent", updated);
        assert!(manager.active().is_empty());
    }

    #[test]
    fn test_overlay_manager_update_preserves_modal() {
        let mut manager = TuiOverlayManager::new();
        manager.register_renderer(Box::new(TestRenderer::modal("palette")));
        manager.set_anchor_context(test_context());

        let overlay = LogicalOverlay::new("modal-1", "palette", Anchor::Center);
        manager.show(overlay, Size::new(80, 24));
        assert!(manager.has_modal());

        // Update should preserve the modal flag
        let updated = LogicalOverlay::new("modal-1", "palette", Anchor::Center)
            .with_state(OverlayState::with_selection(3));
        manager.update("modal-1", updated);
        assert!(manager.has_modal());
        assert_eq!(manager.active()[0].logical.state.selected_index, Some(3));
    }

    #[test]
    fn test_overlay_manager_clear_empty() {
        let mut manager = TuiOverlayManager::new();
        // Clearing an empty manager should be a no-op
        manager.clear();
        assert!(manager.active().is_empty());
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_overlay_manager_interact_select_next() {
        let mut manager = TuiOverlayManager::new();
        manager.register_renderer(Box::new(TestRenderer::new("completion")));
        manager.set_anchor_context(test_context());

        let overlay = LogicalOverlay::new("test-1", "completion", Anchor::Cursor)
            .with_state(OverlayState::with_selection(0));
        manager.show(overlay, Size::new(80, 24));

        let result = manager.interact(Interaction::SelectNext);
        if let InteractionResult::StateUpdate(state) = result {
            assert_eq!(state.selected_index, Some(1)); // 0 + 1
        } else {
            panic!("Expected StateUpdate");
        }
    }

    #[test]
    fn test_overlay_position_screen() {
        let mut manager = TuiOverlayManager::new();
        manager.register_renderer(Box::new(TestRenderer::new("completion")));
        // Anchor::Screen is handled via convert_anchor which maps to TuiAnchor::Screen
        // We need a context with specific screen anchor
        let mut ctx = test_context();
        ctx.screen_size = Size::new(100, 50);
        manager.set_anchor_context(ctx);

        let overlay =
            LogicalOverlay::new("screen-1", "completion", Anchor::Screen { x: 0.5, y: 0.5 });
        manager.show(overlay, Size::new(100, 50));

        let pos = manager.active()[0].position;
        // Screen anchor at (0.5, 0.5) maps to absolute (50, 25)
        assert_eq!(pos.x, 50);
        assert_eq!(pos.y, 25);
    }

    #[test]
    fn test_overlay_position_below_unknown() {
        let mut manager = TuiOverlayManager::new();
        manager.register_renderer(Box::new(TestRenderer::new("completion")));
        manager.set_anchor_context(test_context());

        // Below with unknown overlay falls back to Center
        let overlay =
            LogicalOverlay::new("below-1", "completion", Anchor::Below("unknown".to_string()));
        manager.show(overlay, Size::new(80, 24));

        // Center position: (80-40)/2 = 20, (24-10)/2 = 7
        let pos = manager.active()[0].position;
        assert_eq!(pos.x, 20);
        assert_eq!(pos.y, 7);
    }

    #[test]
    fn test_overlay_position_below_known() {
        let mut manager = TuiOverlayManager::new();
        manager.register_renderer(Box::new(TestRenderer::new("completion")));
        let mut ctx = test_context();
        ctx.register_overlay("other", WindowId::from_raw(500));
        manager.set_anchor_context(ctx);

        // Below with known overlay
        let overlay =
            LogicalOverlay::new("below-1", "completion", Anchor::Below("other".to_string()));
        manager.show(overlay, Size::new(80, 24));

        // Below anchor fallback: position is (0, 1)
        let pos = manager.active()[0].position;
        assert_eq!(pos.x, 0);
        assert_eq!(pos.y, 1);
    }

    #[test]
    fn test_overlay_manager_window_ids_increment() {
        let mut manager = TuiOverlayManager::new();
        manager.register_renderer(Box::new(TestRenderer::new("completion")));
        manager.set_anchor_context(test_context());

        let o1 = LogicalOverlay::new("a", "completion", Anchor::Cursor);
        manager.show(o1, Size::new(80, 24));
        let id1 = manager.window_id("a").unwrap();

        let o2 = LogicalOverlay::new("b", "completion", Anchor::Cursor);
        manager.show(o2, Size::new(80, 24));
        let id2 = manager.window_id("b").unwrap();

        // IDs should be different and incrementing
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_overlay_hide_clears_mappings() {
        let mut manager = TuiOverlayManager::new();
        manager.register_renderer(Box::new(TestRenderer::new("completion")));
        manager.set_anchor_context(test_context());

        let overlay = LogicalOverlay::new("test-1", "completion", Anchor::Cursor);
        manager.show(overlay, Size::new(80, 24));

        let window_id = manager.window_id("test-1").unwrap();
        assert!(manager.overlay_id(window_id).is_some());

        manager.hide("test-1");

        // Both forward and reverse mappings should be gone
        assert!(manager.window_id("test-1").is_none());
        assert!(manager.overlay_id(window_id).is_none());
    }

    #[test]
    fn test_overlay_clear_clears_all_mappings() {
        let mut manager = TuiOverlayManager::new();
        manager.register_renderer(Box::new(TestRenderer::new("completion")));
        manager.set_anchor_context(test_context());

        manager.show(LogicalOverlay::new("a", "completion", Anchor::Cursor), Size::new(80, 24));
        manager.show(LogicalOverlay::new("b", "completion", Anchor::Cursor), Size::new(80, 24));

        assert_eq!(manager.active().len(), 2);
        assert!(manager.window_id("a").is_some());
        assert!(manager.window_id("b").is_some());

        manager.clear();

        assert!(manager.active().is_empty());
        assert!(manager.window_id("a").is_none());
        assert!(manager.window_id("b").is_none());
    }
}
