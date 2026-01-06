//! Jump label rendering
//!
//! Renders jump labels as overlay on the screen using `PluginWindow`.

use std::sync::Arc;

use reovim_core::{
    frame::{Cell, FrameBuffer},
    highlight::Theme,
    plugin::{EditorContext, PluginStateRegistry, PluginWindow, Rect, WindowConfig},
};

use super::state::JumpMatch;

/// Plugin window for rendering jump labels
pub struct JumpLabelWindow;

impl PluginWindow for JumpLabelWindow {
    #[allow(clippy::cast_possible_truncation)]
    fn window_config(
        &self,
        state: &Arc<PluginStateRegistry>,
        ctx: &EditorContext,
    ) -> Option<WindowConfig> {
        // Import SharedJumpState type here since we removed it from use
        use super::state::SharedJumpState;

        // Check if jump mode is showing labels
        // Note: Registry stores Arc<SharedJumpState>, so we retrieve it as Arc<SharedJumpState>
        let is_showing_labels = state
            .with::<Arc<SharedJumpState>, _, _>(|jump_state_arc| {
                jump_state_arc.with(super::state::JumpState::is_showing_labels)
            })
            .unwrap_or(false);

        tracing::debug!(
            "JumpLabelWindow::window_config called, showing_labels={}",
            is_showing_labels
        );

        if !is_showing_labels {
            return None;
        }

        // Full-screen overlay
        Some(WindowConfig {
            bounds: Rect::new(0, 0, ctx.screen_width, ctx.screen_height),
            z_order: 200, // Above editor (100), below popups (300+)
            visible: true,
        })
    }

    #[allow(clippy::cast_possible_truncation)]
    fn render(
        &self,
        state: &Arc<PluginStateRegistry>,
        ctx: &EditorContext,
        buffer: &mut FrameBuffer,
        _bounds: Rect,
        theme: &Theme,
    ) {
        // Import SharedJumpState type here
        use super::state::SharedJumpState;

        // Get matches from shared jump state
        // Note: Registry stores Arc<SharedJumpState>, so we retrieve it as Arc<SharedJumpState>
        let matches: Option<Vec<JumpMatch>> = state
            .with::<Arc<SharedJumpState>, _, _>(|jump_state_arc| {
                jump_state_arc.with(|state| state.get_matches().map(<[JumpMatch]>::to_vec))
            })
            .flatten();

        let Some(matches) = matches else {
            tracing::debug!("JumpLabelWindow::render called but no matches");
            return;
        };

        tracing::debug!("JumpLabelWindow::render called with {} matches", matches.len());

        // Label style (primary yellow labels)
        let label_style = &theme.leap.label_primary;

        // Render each label
        for m in matches {
            // Convert buffer line to screen Y coordinate
            // Screen Y = buffer line - window scroll offset + window anchor Y
            let screen_y = match m.line.checked_sub(u32::from(ctx.active_window_scroll_y)) {
                Some(relative_y) => ctx.active_window_anchor_y + relative_y as u16,
                None => continue, // Match is above visible area
            };

            // Screen X = window anchor X + gutter width + buffer column
            let screen_x =
                ctx.active_window_anchor_x + ctx.active_window_gutter_width + m.col as u16;

            // Check if position is within screen bounds
            if screen_y >= ctx.screen_height || screen_x >= ctx.screen_width {
                continue;
            }

            // Render label characters
            for (i, ch) in m.label.chars().enumerate() {
                let x = screen_x + i as u16;
                if x < ctx.screen_width {
                    let cell = Cell::new(ch, label_style.clone());
                    buffer.set(x, screen_y, cell);
                }
            }
        }
    }
}
