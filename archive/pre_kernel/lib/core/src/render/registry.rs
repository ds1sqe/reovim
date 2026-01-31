//! Registry for render pipeline stages

use std::sync::Arc;

use super::RenderStage;

/// Registry for render pipeline stages
///
/// Plugins register their render stages here during initialization.
/// The rendering system retrieves and executes them in order.
#[derive(Default)]
pub struct RenderStageRegistry {
    stages: Vec<Arc<dyn RenderStage>>,
}

impl RenderStageRegistry {
    /// Create a new empty registry
    #[must_use]
    pub fn new() -> Self {
        Self { stages: Vec::new() }
    }

    /// Register a render stage
    ///
    /// Stages are executed in registration order during rendering.
    pub fn register(&mut self, stage: Arc<dyn RenderStage>) {
        tracing::debug!(stage_name = stage.name(), "Registered render stage");
        self.stages.push(stage);
    }

    /// Get all registered stages
    #[must_use]
    pub fn stages(&self) -> &[Arc<dyn RenderStage>] {
        &self.stages
    }
}
