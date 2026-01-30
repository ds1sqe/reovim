//! Overlay traits for popup UI elements.
//!
//! These traits define how overlays (completion menus, hover info, etc.)
//! are rendered and managed by platform-specific clients.

use crate::{
    Size,
    interaction::{Interaction, InteractionResult},
    rendered::RenderedOverlay,
    wire::LogicalOverlay,
};

/// Trait for rendering a specific type of overlay.
///
/// Platform clients register renderers for different overlay kinds
/// (completion, hover, signature help, etc.). The manager delegates
/// to the appropriate renderer based on the overlay's kind.
///
/// # Object Safety
///
/// This trait is object-safe and can be used as `Box<dyn OverlayRenderer>`.
pub trait OverlayRenderer: Send + Sync {
    /// Check if this renderer handles the given overlay kind.
    fn handles(&self, kind: &str) -> bool;

    /// Measure the size needed to render this overlay.
    ///
    /// Returns the preferred size, constrained by `max_size`.
    fn measure(&self, overlay: &LogicalOverlay, max_size: Size) -> Size;

    /// Handle an interaction with this overlay.
    ///
    /// Returns the result of the interaction (state update, confirm, etc.).
    fn interact(&self, overlay: &LogicalOverlay, interaction: Interaction) -> InteractionResult;

    /// Check if this overlay should be modal (capture all input).
    ///
    /// Default implementation returns `false`.
    fn is_modal(&self, _overlay: &LogicalOverlay) -> bool {
        false
    }

    /// Get the default priority for this overlay kind.
    ///
    /// Higher priority overlays appear on top.
    fn default_priority(&self, _overlay: &LogicalOverlay) -> u32 {
        0
    }
}

/// Trait for managing the overlay stack.
///
/// The overlay manager coordinates multiple overlays, handling
/// z-ordering, focus, and input routing.
pub trait OverlayManager {
    /// Register a renderer for handling overlays.
    fn register_renderer(&mut self, renderer: Box<dyn OverlayRenderer>);

    /// Show an overlay.
    ///
    /// The manager resolves the overlay's anchor to a screen position
    /// and adds it to the stack.
    fn show(&mut self, overlay: LogicalOverlay, screen_size: Size);

    /// Hide an overlay by ID.
    ///
    /// Returns `true` if the overlay was found and hidden.
    fn hide(&mut self, id: &str) -> bool;

    /// Get all active (visible) overlays in z-order.
    fn active(&self) -> &[RenderedOverlay];

    /// Check if any overlay is modal.
    fn has_modal(&self) -> bool;

    /// Route an interaction to the appropriate overlay.
    ///
    /// Returns the result from the topmost overlay that handles
    /// the interaction.
    fn interact(&mut self, interaction: Interaction) -> InteractionResult;

    /// Update an existing overlay's state.
    fn update(&mut self, id: &str, overlay: LogicalOverlay);

    /// Clear all overlays.
    fn clear(&mut self);
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::wire::{Anchor, OverlayState},
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

    impl OverlayRenderer for TestRenderer {
        fn handles(&self, kind: &str) -> bool {
            self.kind == kind
        }

        fn measure(&self, _overlay: &LogicalOverlay, max_size: Size) -> Size {
            // Return a fixed size, constrained by max
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

    #[test]
    fn test_overlay_renderer_handles() {
        let renderer = TestRenderer::new("completion");
        assert!(renderer.handles("completion"));
        assert!(!renderer.handles("hover"));
    }

    #[test]
    fn test_overlay_renderer_measure() {
        let renderer = TestRenderer::new("completion");
        let overlay = LogicalOverlay::new("test", "completion", Anchor::Cursor);

        let size = renderer.measure(&overlay, Size::new(80, 24));
        assert_eq!(size.width, 40);
        assert_eq!(size.height, 10);

        // Constrained by max
        let size = renderer.measure(&overlay, Size::new(20, 5));
        assert_eq!(size.width, 20);
        assert_eq!(size.height, 5);
    }

    #[test]
    fn test_overlay_renderer_interact_confirm() {
        let renderer = TestRenderer::new("completion");
        let overlay = LogicalOverlay::new("test", "completion", Anchor::Cursor);

        let result = renderer.interact(&overlay, Interaction::Confirm);
        assert!(result.closes_overlay());
        if let InteractionResult::Confirm(value) = result {
            assert_eq!(value["selected"], true);
        } else {
            panic!("Expected Confirm result");
        }
    }

    #[test]
    fn test_overlay_renderer_interact_cancel() {
        let renderer = TestRenderer::new("completion");
        let overlay = LogicalOverlay::new("test", "completion", Anchor::Cursor);

        let result = renderer.interact(&overlay, Interaction::Cancel);
        assert!(result.closes_overlay());
        assert!(matches!(result, InteractionResult::Cancel));
    }

    #[test]
    fn test_overlay_renderer_interact_select_next() {
        let renderer = TestRenderer::new("completion");
        let overlay = LogicalOverlay::new("test", "completion", Anchor::Cursor)
            .with_state(OverlayState::with_selection(0));

        let result = renderer.interact(&overlay, Interaction::SelectNext);
        if let InteractionResult::StateUpdate(state) = result {
            assert_eq!(state.selected_index, Some(1));
        } else {
            panic!("Expected StateUpdate result");
        }
    }

    #[test]
    fn test_overlay_renderer_interact_pass_through() {
        let renderer = TestRenderer::new("completion");
        let overlay = LogicalOverlay::new("test", "completion", Anchor::Cursor);

        let result = renderer.interact(&overlay, Interaction::custom("unknown"));
        assert!(!result.was_handled());
        assert!(matches!(result, InteractionResult::PassThrough));
    }

    #[test]
    fn test_overlay_renderer_is_modal() {
        let normal = TestRenderer::new("completion");
        let modal = TestRenderer::modal("command-palette");

        let overlay = LogicalOverlay::new("test", "test", Anchor::Center);
        assert!(!normal.is_modal(&overlay));
        assert!(modal.is_modal(&overlay));
    }

    #[test]
    fn test_overlay_renderer_object_safe() {
        // Verify the trait is object-safe
        let renderer: Box<dyn OverlayRenderer> = Box::new(TestRenderer::new("completion"));
        assert!(renderer.handles("completion"));
    }

    #[test]
    fn test_overlay_renderer_send_sync() {
        // Verify the trait bounds
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<TestRenderer>();
    }
}
