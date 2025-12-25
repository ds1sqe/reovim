//! Animation render stage for the render pipeline
//!
//! This stage applies active animation effects to render data,
//! transforming animated styles into concrete highlights/decorations.

use std::sync::Arc;

use tokio::sync::RwLock;

use crate::{
    component::RenderContext,
    render::{Decoration, DecorationKind, LineHighlight, RenderData, RenderStage},
};

use super::{controller::AnimationState, effect::EffectTarget};

/// Render stage that applies animation effects
///
/// This stage queries the shared `AnimationState` for active effects
/// and applies their resolved styles to the render data.
pub struct AnimationRenderStage {
    /// Shared animation state (read-only access during render)
    state: Arc<RwLock<AnimationState>>,
}

impl AnimationRenderStage {
    /// Create a new animation render stage
    #[must_use]
    pub const fn new(state: Arc<RwLock<AnimationState>>) -> Self {
        Self { state }
    }
}

impl RenderStage for AnimationRenderStage {
    fn transform(&self, mut input: RenderData, _ctx: &RenderContext<'_>) -> RenderData {
        // Try to read animation state without blocking
        // If we can't acquire the lock, skip animation for this frame
        let Ok(state) = self.state.try_read() else {
            return input;
        };

        // Skip if no active effects
        if !state.has_active_effects() {
            return input;
        }

        // Process each effect and apply to render data
        for effect in state.all_effects() {
            let resolved_style = effect.resolve_style();

            match &effect.target {
                EffectTarget::HighlightGroup(_group) => {
                    // Apply to all lines as a background decoration
                    // This creates a subtle effect across the whole buffer
                    // For more targeted effects, use CellRegion
                    if resolved_style.bg.is_some() {
                        for line_decorations in &mut input.decorations {
                            line_decorations.push(Decoration {
                                start_col: 0,
                                end_col: usize::MAX,
                                kind: DecorationKind::Background {
                                    style: resolved_style.clone(),
                                },
                            });
                        }
                    }
                }

                EffectTarget::CellRegion {
                    buffer_id,
                    start_line,
                    start_col,
                    end_line,
                    end_col,
                } => {
                    // Only apply if this is the correct buffer
                    if *buffer_id != input.buffer_id {
                        continue;
                    }

                    let start = *start_line as usize;
                    // Use inclusive range: end_line + 1 so the loop includes the last line
                    let end = ((*end_line as usize) + 1).min(input.highlights.len());

                    for line_idx in start..end {
                        // Calculate column bounds for this line
                        let end_line_idx = (*end_line as usize).min(end.saturating_sub(1));
                        let (col_start, col_end) = if line_idx == start && line_idx == end_line_idx
                        {
                            // Single line region
                            (*start_col as usize, *end_col as usize)
                        } else if line_idx == start {
                            // First line of multi-line region
                            (*start_col as usize, usize::MAX)
                        } else if line_idx == end_line_idx {
                            // Last line of multi-line region
                            (0, *end_col as usize)
                        } else {
                            // Middle lines
                            (0, usize::MAX)
                        };

                        if let Some(line_highlights) = input.highlights.get_mut(line_idx) {
                            line_highlights.push(LineHighlight {
                                start_col: col_start,
                                end_col: col_end,
                                style: resolved_style.clone(),
                            });
                        }
                    }
                }

                EffectTarget::UiElement(_element_id) => {
                    // UI element effects (cursor, statusline, etc.) are handled
                    // separately in the screen rendering code, not through RenderData
                    // This allows effects on elements that aren't part of buffer content
                }
            }
        }

        input
    }

    fn name(&self) -> &'static str {
        "animation"
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::{
            animation::{AnimatedStyle, Effect, EffectId},
            render::Bounds,
        },
        reovim_sys::style::Color,
    };

    fn create_test_render_data() -> RenderData {
        RenderData {
            lines: vec!["line 1".to_string(), "line 2".to_string()],
            visibility: vec![
                crate::render::LineVisibility::Visible,
                crate::render::LineVisibility::Visible,
            ],
            highlights: vec![vec![], vec![]],
            decorations: vec![vec![], vec![]],
            buffer_id: 1,
            window_id: 1,
            window_bounds: Bounds {
                x: 0,
                y: 0,
                width: 80,
                height: 24,
            },
            cursor: (0, 0),
        }
    }

    #[test]
    fn test_stage_no_effects() {
        let anim_state = Arc::new(RwLock::new(AnimationState::default()));
        let render_stage = AnimationRenderStage::new(anim_state);

        let input = create_test_render_data();
        let theme = crate::highlight::Theme::dark();
        let ctx = RenderContext {
            screen_width: 80,
            screen_height: 24,
            theme: Box::leak(Box::new(theme)),
            color_mode: crate::highlight::ColorMode::TrueColor,
            tab_line_offset: 0,
            state: None,
            modifier_style: None,
            modifier_behavior: None,
        };

        let output = render_stage.transform(input, &ctx);

        // No effects means no changes
        assert!(output.decorations[0].is_empty());
        assert!(output.decorations[1].is_empty());
    }

    #[tokio::test]
    async fn test_stage_with_cell_region_effect() {
        let anim_state = Arc::new(RwLock::new(AnimationState::default()));

        // Add an effect targeting a cell region
        {
            let mut state_guard = anim_state.write().await;
            let effect = Effect::new(
                EffectId::new(1),
                EffectTarget::CellRegion {
                    buffer_id: 1,
                    start_line: 0,
                    start_col: 0,
                    end_line: 1,
                    end_col: 6,
                },
                AnimatedStyle::pulse_bg(Color::Black, Color::White, 1000),
            );

            // Directly insert into state for testing
            // In real usage, this goes through the controller
            state_guard
                .effects
                .entry(effect.target.clone())
                .or_default()
                .push(effect);
        }

        let render_stage = AnimationRenderStage::new(Arc::clone(&anim_state));
        let input = create_test_render_data();
        let theme = crate::highlight::Theme::dark();
        let ctx = RenderContext {
            screen_width: 80,
            screen_height: 24,
            theme: Box::leak(Box::new(theme)),
            color_mode: crate::highlight::ColorMode::TrueColor,
            tab_line_offset: 0,
            state: None,
            modifier_style: None,
            modifier_behavior: None,
        };

        let output = render_stage.transform(input, &ctx);

        // Effect should have added a highlight to line 0
        assert!(!output.highlights[0].is_empty());
    }
}
