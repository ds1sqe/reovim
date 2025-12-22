//! Render pipeline stage trait

use super::RenderData;
use crate::component::RenderContext;

/// Pipeline stage that transforms render data
///
/// Each stage receives render data and returns augmented data.
/// Stages should be immutable transformations.
pub trait RenderStage: Send + Sync {
    /// Transform render data
    ///
    /// Takes input data and returns modified data for the next stage.
    fn transform(&self, input: RenderData, ctx: &RenderContext<'_>) -> RenderData;

    /// Stage name for debugging
    fn name(&self) -> &'static str;
}
