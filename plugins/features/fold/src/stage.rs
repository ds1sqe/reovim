//! Fold render stage for pipeline integration

use std::sync::Arc;

use reovim_core::{
    component::RenderContext,
    render::{LineVisibility, RenderData, RenderStage},
    visibility::{BufferVisibilitySource, VisibilityQuery},
};

use crate::state::SharedFoldManager;

/// Fold render stage - sets line visibility for folded ranges
pub struct FoldRenderStage {
    fold_manager: Arc<SharedFoldManager>,
}

impl FoldRenderStage {
    /// Create new fold render stage
    pub fn new(fold_manager: Arc<SharedFoldManager>) -> Self {
        Self { fold_manager }
    }
}

impl RenderStage for FoldRenderStage {
    fn transform(&self, mut input: RenderData, _ctx: &RenderContext<'_>) -> RenderData {
        let buffer_id = input.buffer_id;

        // Update visibility for each line based on fold state
        for (line_idx, visibility) in input.visibility.iter_mut().enumerate() {
            let line_num = line_idx as u32;

            // Check if line is hidden by a fold
            if self.fold_manager.is_hidden(buffer_id, VisibilityQuery::Line(line_num)) {
                *visibility = LineVisibility::Hidden;
            }
            // Check if line is a fold marker
            else if let Some(marker) = self.fold_manager.get_marker(buffer_id, VisibilityQuery::Line(line_num)) {
                *visibility = LineVisibility::FoldMarker {
                    preview: marker.preview.clone(),
                    hidden_lines: marker.hidden_count,
                };
            }
            // Otherwise keep as Visible (default from RenderData::from_buffer)
        }

        input
    }

    fn name(&self) -> &'static str {
        "fold"
    }
}
