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

        // Calculate viewport boundaries
        let viewport_bottom = ctx.active_window_anchor_y + ctx.active_window_height;

        // Render each label
        for m in matches {
            // Convert buffer line to screen Y coordinate
            // Screen Y = buffer line - window scroll offset + window anchor Y
            let screen_y = match m.line.checked_sub(u32::from(ctx.active_window_scroll_y)) {
                Some(relative_y) => ctx.active_window_anchor_y + relative_y as u16,
                None => continue, // Match is above visible area
            };

            // Check if within viewport boundaries (only show labels for visible matches)
            if screen_y >= viewport_bottom {
                continue;
            }

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

#[cfg(test)]
mod tests {
    use {
        super::*,
        reovim_core::{
            highlight::ColorMode,
            modd::{ComponentId, EditMode, SubMode},
        },
    };

    use crate::jump::state::SharedJumpState;

    /// Create a mock `EditorContext` for testing
    fn mock_editor_context(
        screen_width: u16,
        screen_height: u16,
        anchor_y: u16,
        window_height: u16,
        scroll_y: u16,
    ) -> EditorContext {
        EditorContext {
            screen_width,
            screen_height,
            tab_line_height: 0,
            status_line_height: 1,
            left_offset: 0,
            right_offset: 0,
            edit_mode: EditMode::Normal,
            sub_mode: SubMode::None,
            focused_component: ComponentId::EDITOR,
            active_buffer_id: 0,
            buffer_count: 1,
            active_window_anchor_x: 0,
            active_window_anchor_y: anchor_y,
            active_window_gutter_width: 0,
            active_window_scroll_y: scroll_y,
            active_window_height: window_height,
            cursor_col: 0,
            cursor_row: 0,
            active_buffer_content: None,
            color_mode: ColorMode::TrueColor,
            pending_keys: String::new(),
        }
    }

    /// Create a mock `PluginStateRegistry` with jump state containing matches
    fn mock_registry_with_matches(matches: Vec<JumpMatch>) -> Arc<PluginStateRegistry> {
        let registry = Arc::new(PluginStateRegistry::new());
        let jump_state = Arc::new(SharedJumpState::new());

        // Set up jump state with matches
        jump_state.with_mut(|state| {
            state.start_multi_char(0, 10, 5, crate::jump::Direction::Forward, vec![], None);
            state.handle_first_char('a'); // Transition to WaitingSecondChar phase
            state.show_labels("ab".to_string(), matches);
        });

        // Register the jump state
        registry.register(jump_state);
        registry
    }

    /// Helper to check if a cell at (x, y) has been rendered
    fn is_cell_rendered(buffer: &FrameBuffer, x: u16, y: u16) -> bool {
        // Check if the cell at (x, y) is not the default empty cell
        // We just check if any non-space character was rendered at this position
        buffer.get(x, y).is_some_and(|cell| cell.char != ' ')
    }

    #[test]
    fn test_viewport_filters_matches_below_viewport() {
        // Setup: viewport from y=0 to y=10 (height=10)
        let ctx = mock_editor_context(80, 24, 0, 10, 0);
        let mut buffer = FrameBuffer::new(80, 24);
        let theme = Theme::default();

        // Matches at lines: 5 (visible), 15 (below viewport), 25 (below viewport)
        let matches = vec![
            JumpMatch::new(5, 0, "s".to_string(), 5),
            JumpMatch::new(15, 0, "f".to_string(), 15),
            JumpMatch::new(25, 0, "n".to_string(), 25),
        ];
        let registry = mock_registry_with_matches(matches);

        let window = JumpLabelWindow;
        let bounds = Rect::new(0, 0, 80, 24);
        window.render(&registry, &ctx, &mut buffer, bounds, &theme);

        // Line 5: should be rendered (screen_y = 0 + 5 = 5, viewport_bottom = 0 + 10 = 10)
        assert!(
            is_cell_rendered(&buffer, 0, 5),
            "Match at line 5 should be rendered (within viewport)"
        );

        // Line 15: should NOT be rendered (screen_y = 0 + 15 = 15, >= viewport_bottom 10)
        assert!(
            !is_cell_rendered(&buffer, 0, 15),
            "Match at line 15 should be filtered (below viewport)"
        );

        // Line 25: should NOT be rendered (screen_y = 0 + 25 = 25, >= viewport_bottom 10)
        assert!(
            !is_cell_rendered(&buffer, 0, 25),
            "Match at line 25 should be filtered (below viewport)"
        );
    }

    #[test]
    fn test_viewport_filters_with_scroll_offset() {
        // Setup: viewport from y=1 to y=21 (height=20), scrolled to line 100
        let ctx = mock_editor_context(80, 24, 1, 20, 100);
        let mut buffer = FrameBuffer::new(80, 24);
        let theme = Theme::default();

        // Matches at buffer lines: 105 (visible), 125 (below viewport)
        // Screen positions: 105 - 100 + 1 = 6, 125 - 100 + 1 = 26
        let matches = vec![
            JumpMatch::new(105, 0, "s".to_string(), 5),
            JumpMatch::new(125, 0, "f".to_string(), 25),
        ];
        let registry = mock_registry_with_matches(matches);

        let window = JumpLabelWindow;
        let bounds = Rect::new(0, 0, 80, 24);
        window.render(&registry, &ctx, &mut buffer, bounds, &theme);

        // Line 105: screen_y = 1 + (105 - 100) = 6, viewport_bottom = 1 + 20 = 21
        // Should be rendered (6 < 21)
        assert!(
            is_cell_rendered(&buffer, 0, 6),
            "Match at line 105 should be rendered (within viewport)"
        );

        // Line 125: screen_y = 1 + (125 - 100) = 26, viewport_bottom = 21
        // Should NOT be rendered (26 >= 21)
        assert!(
            !is_cell_rendered(&buffer, 0, 26),
            "Match at line 125 should be filtered (below viewport)"
        );
    }

    #[test]
    fn test_viewport_edge_case_match_at_viewport_boundary() {
        // Setup: viewport from y=0 to y=10 (height=10)
        let ctx = mock_editor_context(80, 24, 0, 10, 0);
        let mut buffer = FrameBuffer::new(80, 24);
        let theme = Theme::default();

        // Matches exactly at viewport boundaries
        let matches = vec![
            JumpMatch::new(0, 0, "s".to_string(), 0),   // Top edge
            JumpMatch::new(9, 0, "f".to_string(), 9),   // Just inside bottom
            JumpMatch::new(10, 0, "n".to_string(), 10), // Exactly at bottom (should be filtered)
        ];
        let registry = mock_registry_with_matches(matches);

        let window = JumpLabelWindow;
        let bounds = Rect::new(0, 0, 80, 24);
        window.render(&registry, &ctx, &mut buffer, bounds, &theme);

        // Line 0: should be rendered (screen_y = 0, < viewport_bottom 10)
        assert!(
            is_cell_rendered(&buffer, 0, 0),
            "Match at line 0 should be rendered (top edge of viewport)"
        );

        // Line 9: should be rendered (screen_y = 9, < viewport_bottom 10)
        assert!(
            is_cell_rendered(&buffer, 0, 9),
            "Match at line 9 should be rendered (just inside viewport)"
        );

        // Line 10: should NOT be rendered (screen_y = 10, >= viewport_bottom 10)
        assert!(
            !is_cell_rendered(&buffer, 0, 10),
            "Match at line 10 should be filtered (exactly at viewport_bottom boundary)"
        );
    }

    #[test]
    fn test_viewport_small_window() {
        // Setup: very small viewport (height=5)
        let ctx = mock_editor_context(80, 24, 0, 5, 0);
        let mut buffer = FrameBuffer::new(80, 24);
        let theme = Theme::default();

        // Matches within and outside small viewport
        let matches = vec![
            JumpMatch::new(2, 0, "s".to_string(), 2),   // Inside
            JumpMatch::new(4, 0, "f".to_string(), 4),   // Inside (last visible line)
            JumpMatch::new(5, 0, "n".to_string(), 5),   // Outside
            JumpMatch::new(10, 0, "j".to_string(), 10), // Outside
        ];
        let registry = mock_registry_with_matches(matches);

        let window = JumpLabelWindow;
        let bounds = Rect::new(0, 0, 80, 24);
        window.render(&registry, &ctx, &mut buffer, bounds, &theme);

        // Lines 2 and 4: should be rendered
        assert!(
            is_cell_rendered(&buffer, 0, 2),
            "Match at line 2 should be rendered in small viewport"
        );
        assert!(
            is_cell_rendered(&buffer, 0, 4),
            "Match at line 4 should be rendered in small viewport"
        );

        // Lines 5 and 10: should NOT be rendered
        assert!(
            !is_cell_rendered(&buffer, 0, 5),
            "Match at line 5 should be filtered in small viewport"
        );
        assert!(
            !is_cell_rendered(&buffer, 0, 10),
            "Match at line 10 should be filtered in small viewport"
        );
    }

    #[test]
    fn test_viewport_zero_height_filters_all() {
        // Setup: viewport with height=0 (edge case)
        let ctx = mock_editor_context(80, 24, 0, 0, 0);
        let mut buffer = FrameBuffer::new(80, 24);
        let theme = Theme::default();

        // Any matches should be filtered with zero height viewport
        let matches = vec![
            JumpMatch::new(0, 0, "s".to_string(), 0),
            JumpMatch::new(5, 0, "f".to_string(), 5),
        ];
        let registry = mock_registry_with_matches(matches);

        let window = JumpLabelWindow;
        let bounds = Rect::new(0, 0, 80, 24);
        window.render(&registry, &ctx, &mut buffer, bounds, &theme);

        // All matches should be filtered when viewport_bottom = anchor_y + 0 = 0
        assert!(
            !is_cell_rendered(&buffer, 0, 0),
            "Match should be filtered when viewport height is 0"
        );
        assert!(
            !is_cell_rendered(&buffer, 0, 5),
            "Match should be filtered when viewport height is 0"
        );
    }

    #[test]
    fn test_viewport_with_window_anchor_offset() {
        // Setup: viewport starts at y=5, height=15 (covers y=5 to y=20)
        let ctx = mock_editor_context(80, 24, 5, 15, 0);
        let mut buffer = FrameBuffer::new(80, 24);
        let theme = Theme::default();

        // Matches at various buffer lines
        let matches = vec![
            JumpMatch::new(3, 0, "s".to_string(), 3), // screen_y = 5 + 3 = 8 (visible)
            JumpMatch::new(10, 0, "f".to_string(), 10), // screen_y = 5 + 10 = 15 (visible)
            JumpMatch::new(19, 0, "n".to_string(), 19), // screen_y = 5 + 19 = 24 (outside)
        ];
        let registry = mock_registry_with_matches(matches);

        let window = JumpLabelWindow;
        let bounds = Rect::new(0, 0, 80, 24);
        window.render(&registry, &ctx, &mut buffer, bounds, &theme);

        // viewport_bottom = 5 + 15 = 20
        // Line 3: screen_y = 8, < 20 (visible)
        assert!(
            is_cell_rendered(&buffer, 0, 8),
            "Match at line 3 should be rendered with anchor offset"
        );

        // Line 10: screen_y = 15, < 20 (visible)
        assert!(
            is_cell_rendered(&buffer, 0, 15),
            "Match at line 10 should be rendered with anchor offset"
        );

        // Line 19: screen_y = 24, >= 20 (outside viewport and screen bounds)
        assert!(
            !is_cell_rendered(&buffer, 0, 24),
            "Match at line 19 should be filtered (beyond screen)"
        );
    }

    #[test]
    fn test_viewport_all_matches_visible() {
        // Setup: large viewport (height=20) that shows all matches
        let ctx = mock_editor_context(80, 24, 0, 20, 0);
        let mut buffer = FrameBuffer::new(80, 24);
        let theme = Theme::default();

        // All matches within viewport
        let matches = vec![
            JumpMatch::new(2, 0, "s".to_string(), 2),
            JumpMatch::new(5, 0, "f".to_string(), 5),
            JumpMatch::new(10, 0, "n".to_string(), 10),
            JumpMatch::new(15, 0, "j".to_string(), 15),
        ];
        let registry = mock_registry_with_matches(matches);

        let window = JumpLabelWindow;
        let bounds = Rect::new(0, 0, 80, 24);
        window.render(&registry, &ctx, &mut buffer, bounds, &theme);

        // viewport_bottom = 0 + 20 = 20
        // All matches should be rendered
        assert!(is_cell_rendered(&buffer, 0, 2), "Match at line 2 should be rendered");
        assert!(is_cell_rendered(&buffer, 0, 5), "Match at line 5 should be rendered");
        assert!(is_cell_rendered(&buffer, 0, 10), "Match at line 10 should be rendered");
        assert!(is_cell_rendered(&buffer, 0, 15), "Match at line 15 should be rendered");
    }
}
