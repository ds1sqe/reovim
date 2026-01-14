//! Pair render stage for pipeline integration
//!
//! Applies bracket highlighting to the render output:
//! - Rainbow colors based on nesting depth
//! - Bold + underline when cursor is directly on a bracket
//! - Red underline for unmatched brackets

use std::sync::Arc;

use reovim_core::{
    component::RenderContext,
    render::{LineHighlight, RenderData, RenderStage},
};

use crate::{rainbow::content_hash, state::SharedPairState};

/// Number of rainbow colors (cycles after this)
const RAINBOW_COLOR_COUNT: usize = 6;

/// Pair render stage - adds rainbow bracket and matched pair highlights
pub struct PairRenderStage {
    pair_state: Arc<SharedPairState>,
}

impl PairRenderStage {
    /// Create new pair render stage
    pub const fn new(pair_state: Arc<SharedPairState>) -> Self {
        Self { pair_state }
    }
}

impl RenderStage for PairRenderStage {
    fn transform(&self, mut input: RenderData, ctx: &RenderContext<'_>) -> RenderData {
        let buffer_id = input.buffer_id;

        // Compute content hash for cache validation
        let content = input.lines.join("\n");
        let hash = content_hash(&content);

        // Get cursor position directly from render data (reliable, always up-to-date)
        let cursor = input.cursor;

        // Ensure bracket depths are computed
        self.pair_state.ensure_computed(buffer_id, &content, hash);

        // Compute matched pair based on current cursor position
        self.pair_state
            .compute_matched_pair_with_cursor(buffer_id, cursor);

        // Get bracket styles from theme
        let bracket_styles = &ctx.theme.brackets;

        // Only highlight the matched pair that contains the cursor
        if let Some(matched) = self.pair_state.get_matched_pair(buffer_id) {
            // Check if cursor is on either bracket
            let open_pos = (matched.open.line, matched.open.col);
            let close_pos = (matched.close.line, matched.close.col);
            let cursor_on_bracket = cursor == open_pos || cursor == close_pos;

            // Use rainbow color based on depth
            let color_idx = matched.open.depth % RAINBOW_COLOR_COUNT;
            // When cursor is on bracket: add bold + underline to rainbow color
            // When cursor is inside: use rainbow color only
            let style = if cursor_on_bracket {
                bracket_styles.rainbow[color_idx].clone().bold().underline()
            } else {
                bracket_styles.rainbow[color_idx].clone()
            };

            // Highlight opening bracket
            if matched.open.line < input.highlights.len() {
                input.highlights[matched.open.line].push(LineHighlight {
                    start_col: matched.open.col,
                    end_col: matched.open.col + 1,
                    style: style.clone(),
                });
            }

            // Highlight closing bracket
            if matched.close.line < input.highlights.len() {
                input.highlights[matched.close.line].push(LineHighlight {
                    start_col: matched.close.col,
                    end_col: matched.close.col + 1,
                    style,
                });
            }
        }

        // Highlight unmatched brackets with warning style (underline + red)
        if let Some(brackets) = self.pair_state.get_brackets(buffer_id) {
            for ((line, col), info) in brackets.iter() {
                if info.depth == usize::MAX {
                    // Apply unmatched warning style from theme
                    if *line < input.highlights.len() {
                        input.highlights[*line].push(LineHighlight {
                            start_col: *col,
                            end_col: *col + 1,
                            style: bracket_styles.unmatched.clone(),
                        });
                    }
                }
            }
        }

        input
    }

    fn name(&self) -> &'static str {
        "pair"
    }
}
