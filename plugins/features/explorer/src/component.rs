//! UIComponent implementation for Explorer
//!
//! This component handles input routing for the explorer plugin.
//! Rendering is handled by the WindowProvider system.

use reovim_core::{
    component::RenderContext,
    event::InnerEvent,
    frame::FrameBuffer,
    interactor::InputResult,
    modd::ModeState,
    screen::{z_order, LayerBounds},
    ui_component::{ComponentId, UIComponent},
};

use super::COMPONENT_ID;

/// Explorer UI component
///
/// This component routes input events to the explorer's focus handler.
/// Rendering is delegated to the ExplorerWindowProvider.
#[derive(Debug)]
pub struct ExplorerComponent;

impl UIComponent for ExplorerComponent {
    fn id(&self) -> ComponentId {
        COMPONENT_ID
    }

    fn display_name(&self) -> &'static str {
        "EXPLORER"
    }

    fn icon(&self) -> Option<&'static str> {
        Some("")
    }

    fn z_order(&self) -> u8 {
        z_order::BASE
    }

    fn is_visible(&self, _ctx: &RenderContext<'_>) -> bool {
        // Visibility is controlled by the window system
        true
    }

    fn bounds(&self, _ctx: &RenderContext<'_>) -> LayerBounds {
        // Bounds are determined by the window provider
        LayerBounds {
            x: 0,
            y: 0,
            width: 0,
            height: 0,
        }
    }

    fn render_to_frame(&self, _buffer: &mut FrameBuffer, _ctx: &RenderContext<'_>) {
        // Rendering is handled by ExplorerWindowProvider
    }

    fn is_focusable(&self) -> bool {
        true
    }

    fn handle_insert_char(&mut self, c: char, _mode_state: &ModeState) -> InputResult {
        // Route character input through FocusInput event to the focus handler
        tracing::debug!("ExplorerComponent: routing char '{}' to focus handler", c);
        InputResult::SendEvent(InnerEvent::FocusInput {
            char: Some(c),
            delete: false,
            clear_landing: false,
        })
    }

    fn handle_delete_backward(&mut self, _mode_state: &ModeState) -> InputResult {
        // Route backspace through FocusInput event to the focus handler
        tracing::debug!("ExplorerComponent: routing backspace to focus handler");
        InputResult::SendEvent(InnerEvent::FocusInput {
            char: None,
            delete: true,
            clear_landing: false,
        })
    }

    fn captures_input(&self) -> bool {
        // Explorer captures all input when in Interactor mode
        true
    }
}
