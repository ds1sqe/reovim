//! UIComponent implementation for Explorer
//!
//! This component handles input for the explorer plugin.
//! Rendering is handled by the WindowProvider system.

use reovim_core::{
    component::RenderContext,
    frame::FrameBuffer,
    interactor::InputResult,
    modd::ModeState,
    plugin::PluginStateRegistry,
    screen::{LayerBounds, z_order},
    ui_component::{ComponentId, UIComponent},
};

use super::{COMPONENT_ID, state::ExplorerState};

/// Explorer UI component
///
/// This component handles input directly by manipulating explorer state.
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

    fn handle_insert_char(
        &mut self,
        c: char,
        _mode_state: &ModeState,
        state: &PluginStateRegistry,
    ) -> InputResult {
        // Directly manipulate explorer state for character input
        tracing::debug!("ExplorerComponent: handling char '{}'", c);

        state
            .with_mut::<ExplorerState, _, _>(|explorer| {
                if !explorer.input_buffer.is_empty() || explorer.message.is_some() {
                    // In input mode - add character to buffer
                    explorer.input_buffer.push(c);
                    tracing::debug!(
                        "ExplorerComponent: input_buffer now: '{}'",
                        explorer.input_buffer
                    );
                    InputResult::Handled
                } else {
                    // Not in input mode - don't handle
                    InputResult::NotHandled
                }
            })
            .unwrap_or(InputResult::NotHandled)
    }

    fn handle_delete_backward(
        &mut self,
        _mode_state: &ModeState,
        state: &PluginStateRegistry,
    ) -> InputResult {
        // Directly manipulate explorer state for backspace
        tracing::debug!("ExplorerComponent: handling backspace");

        state
            .with_mut::<ExplorerState, _, _>(|explorer| {
                if !explorer.input_buffer.is_empty() {
                    // In input mode - remove last character
                    explorer.input_buffer.pop();
                    tracing::debug!(
                        "ExplorerComponent: input_buffer now: '{}'",
                        explorer.input_buffer
                    );
                    InputResult::Handled
                } else {
                    // Not in input mode - don't handle
                    InputResult::NotHandled
                }
            })
            .unwrap_or(InputResult::NotHandled)
    }

    fn captures_input(&self) -> bool {
        // Explorer captures all input when in Interactor mode
        true
    }
}
