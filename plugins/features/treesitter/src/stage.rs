//! Treesitter render stage for syntax highlighting

use std::sync::Arc;

use reovim_core::{
    component::RenderContext,
    render::{RenderData, RenderStage},
};

use crate::state::SharedTreesitterManager;

/// Treesitter render stage - populates syntax highlighting
///
/// This stage generates highlights from treesitter parse trees
/// and populates the highlights field in RenderData for each line.
pub struct TreesitterRenderStage {
    _manager: Arc<SharedTreesitterManager>,
}

impl TreesitterRenderStage {
    /// Create new treesitter render stage
    pub fn new(_manager: Arc<SharedTreesitterManager>) -> Self {
        Self { _manager }
    }
}

impl RenderStage for TreesitterRenderStage {
    fn transform(&self, input: RenderData, _ctx: &RenderContext<'_>) -> RenderData {
        // TODO: Implement highlighting from SharedTreesitterManager
        // For now, pass through unchanged (highlighting still works via old HighlightStore path)
        input
    }

    fn name(&self) -> &'static str {
        "treesitter"
    }
}
