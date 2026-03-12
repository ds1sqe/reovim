//! Integration helpers for the annotation system.
//!
//! This module provides high-level utilities for integrating the annotation
//! system into rendering pipelines. It bridges sources, stores, and presenters.
//!
//! # Example
//!
//! ```ignore
//! use reovim_driver_display::annotation::{
//!     AnnotationSourceRegistry, GutterRenderer,
//! };
//!
//! // Create a gutter renderer with sources
//! let mut renderer = GutterRenderer::new();
//! renderer.register_source(Box::new(line_number_source));
//! renderer.register_presenter(Arc::new(line_number_presenter));
//!
//! // Render gutter for visible lines
//! let cells = renderer.render(buffer_id, 0..24, &context);
//! ```

use std::sync::Arc;

use reovim_kernel::api::v1::{BufferId, MultiServiceRegistry, ServiceKey};

use super::{
    AnnotationContext, AnnotationPresenter, AnnotationSource, AnnotationStore, ComposedLine,
    GutterComposer, GutterConfig, PresenterContext, PresenterRegistry, SourceId,
};

// ============================================================================
// Source Registry
// ============================================================================

/// Key for annotation source lookup.
///
/// Used to register and lookup annotation sources in a registry.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AnnotationSourceKey(pub &'static str);

impl ServiceKey for AnnotationSourceKey {
    fn service_name() -> &'static str {
        "AnnotationSource"
    }
}

impl AnnotationSourceKey {
    /// Create a new source key.
    #[must_use]
    pub const fn new(name: &'static str) -> Self {
        Self(name)
    }
}

/// Registry for annotation sources.
///
/// Uses the kernel's `MultiServiceRegistry` pattern for type-safe lookup.
pub type AnnotationSourceRegistry = MultiServiceRegistry<AnnotationSourceKey, dyn AnnotationSource>;

// ============================================================================
// Gutter Renderer Registry
// ============================================================================

/// Key for gutter renderer lookup.
///
/// Used to register and lookup gutter renderers in a service registry.
/// Following the pattern from Epic #417 (`ServiceRegistry` pattern).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GutterRendererKey {
    /// Default gutter renderer (line numbers, signs, fold markers, etc.)
    Default,
}

impl ServiceKey for GutterRendererKey {
    fn service_name() -> &'static str {
        "GutterRenderer"
    }
}

/// Registry for gutter renderers.
///
/// Uses the kernel's `MultiServiceRegistry` pattern for type-safe lookup.
/// Modules register their `GutterRenderer` instances during initialization,
/// and the runner queries them when rendering screen content.
pub type GutterRendererRegistry = MultiServiceRegistry<GutterRendererKey, GutterRenderer>;

// ============================================================================
// Gutter Renderer
// ============================================================================

/// High-level gutter renderer that integrates sources, store, and presenters.
///
/// This is the main integration point for using the annotation system.
/// It manages the lifecycle of sources, stores annotations, and coordinates
/// rendering through presenters.
///
/// # Thread Safety
///
/// The renderer is `Send + Sync` and can be shared across threads.
/// Internal mutation is handled through interior mutability.
pub struct GutterRenderer {
    /// Registered annotation sources.
    sources: Vec<Arc<dyn AnnotationSource>>,
    /// Presenter registry for rendering.
    presenters: PresenterRegistry,
    /// Gutter configuration.
    config: GutterConfig,
}

impl GutterRenderer {
    /// Create a new gutter renderer with default configuration.
    #[must_use]
    pub fn new() -> Self {
        Self {
            sources: Vec::new(),
            presenters: PresenterRegistry::new(),
            config: GutterConfig::default_line_numbers(),
        }
    }

    /// Create a renderer with custom configuration.
    #[must_use]
    pub fn with_config(config: GutterConfig) -> Self {
        Self {
            sources: Vec::new(),
            presenters: PresenterRegistry::new(),
            config,
        }
    }

    /// Register an annotation source.
    pub fn register_source(&mut self, source: Arc<dyn AnnotationSource>) {
        self.sources.push(source);
    }

    /// Register a presenter.
    pub fn register_presenter(&mut self, presenter: Arc<dyn AnnotationPresenter>) {
        self.presenters.register(presenter);
    }

    /// Set the gutter configuration.
    pub fn set_config(&mut self, config: GutterConfig) {
        self.config = config;
    }

    /// Get the current configuration.
    #[must_use]
    pub const fn config(&self) -> &GutterConfig {
        &self.config
    }

    /// Render gutter for a range of lines.
    ///
    /// # Arguments
    ///
    /// * `buffer_id` - The buffer to render for
    /// * `range` - Line range to render (exclusive end)
    /// * `context` - Annotation context with buffer state
    ///
    /// # Returns
    ///
    /// A vector of composed lines, one per line in the range.
    #[must_use]
    pub fn render(
        &self,
        buffer_id: BufferId,
        range: std::ops::Range<usize>,
        context: &AnnotationContext,
    ) -> Vec<ComposedLine> {
        // Collect annotations from all sources
        let mut store = AnnotationStore::new();
        for source in &self.sources {
            let annotations = source.annotations(buffer_id, range.clone(), context);
            store.replace_source(SourceId::new(source.id()), annotations);
        }

        // Create composer
        let composer = GutterComposer::new(&store, &self.presenters, &self.config);

        // Build presenter context
        let presenter_ctx = PresenterContext::new(context.total_lines, context.cursor_line, false);

        // Compose gutter for range
        composer.compose_range(range.start, range.end, &presenter_ctx)
    }

    /// Calculate the total gutter width.
    ///
    /// # Arguments
    ///
    /// * `context` - Context with total lines (for line number width calculation)
    #[must_use]
    pub fn total_width(&self, context: &AnnotationContext) -> usize {
        // Create empty store just to get width
        let store = AnnotationStore::new();
        let composer = GutterComposer::new(&store, &self.presenters, &self.config);
        let presenter_ctx = PresenterContext::new(context.total_lines, context.cursor_line, false);
        composer.total_width(&presenter_ctx)
    }

    /// Check if any sources have annotations for the buffer.
    #[must_use]
    pub fn has_annotations(&self, buffer_id: BufferId) -> bool {
        self.sources.iter().any(|s| s.has_annotations(buffer_id))
    }

    /// Get the number of registered sources.
    #[must_use]
    pub fn source_count(&self) -> usize {
        self.sources.len()
    }

    /// Get the number of registered presenters.
    #[must_use]
    pub fn presenter_count(&self) -> usize {
        self.presenters.len()
    }
}

impl Default for GutterRenderer {
    fn default() -> Self {
        Self::new()
    }
}

// Ensure GutterRenderer is Send + Sync
const _: () = {
    #[cfg_attr(coverage_nightly, coverage(off))]
    const fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<GutterRenderer>();
};

#[cfg(test)]
#[path = "integration_tests.rs"]
mod tests;
