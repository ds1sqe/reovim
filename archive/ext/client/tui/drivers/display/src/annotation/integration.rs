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

use std::sync::{Arc, RwLock};

use reovim_kernel::api::v1::{BufferId, MultiServiceRegistry, ServiceKey};

use super::{
    AnnotationContext, AnnotationPresenter, AnnotationSource, AnnotationStore, ComposedLine,
    GutterComposer, GutterConfig, PresenterContext, PresenterRegistry, SourceId,
};

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
    /// Registered annotation sources (interior mutability for multi-module registration).
    sources: RwLock<Vec<Arc<dyn AnnotationSource>>>,
    /// Presenter registry for rendering (interior mutability for multi-module registration).
    presenters: RwLock<PresenterRegistry>,
    /// Gutter configuration.
    config: RwLock<GutterConfig>,
}

impl GutterRenderer {
    /// Create a new gutter renderer with default configuration.
    #[must_use]
    pub fn new() -> Self {
        Self {
            sources: RwLock::new(Vec::new()),
            presenters: RwLock::new(PresenterRegistry::new()),
            config: RwLock::new(GutterConfig::default_line_numbers()),
        }
    }

    /// Create a renderer with custom configuration.
    #[must_use]
    pub fn with_config(config: GutterConfig) -> Self {
        Self {
            sources: RwLock::new(Vec::new()),
            presenters: RwLock::new(PresenterRegistry::new()),
            config: RwLock::new(config),
        }
    }

    /// Register an annotation source.
    ///
    /// Uses interior mutability so sources can be added after the renderer
    /// is stored in a `ServiceRegistry` as `Arc<GutterRenderer>`.
    ///
    /// # Panics
    ///
    /// Panics if the internal lock is poisoned.
    pub fn register_source(&self, source: Arc<dyn AnnotationSource>) {
        self.sources
            .write()
            .expect("sources lock poisoned")
            .push(source);
    }

    /// Register a presenter.
    ///
    /// Uses interior mutability so presenters can be added after the renderer
    /// is stored in a `ServiceRegistry` as `Arc<GutterRenderer>`.
    ///
    /// # Panics
    ///
    /// Panics if the internal lock is poisoned.
    pub fn register_presenter(&self, presenter: Arc<dyn AnnotationPresenter>) {
        self.presenters
            .write()
            .expect("presenters lock poisoned")
            .register(presenter);
    }

    /// Set the gutter configuration.
    ///
    /// # Panics
    ///
    /// Panics if the internal lock is poisoned.
    pub fn set_config(&self, config: GutterConfig) {
        *self.config.write().expect("config lock poisoned") = config;
    }

    /// Get a clone of the current configuration.
    ///
    /// # Panics
    ///
    /// Panics if the internal lock is poisoned.
    #[must_use]
    pub fn config(&self) -> GutterConfig {
        self.config.read().expect("config lock poisoned").clone()
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
    ///
    /// # Panics
    ///
    /// Panics if any internal lock is poisoned.
    #[must_use]
    pub fn render(
        &self,
        buffer_id: BufferId,
        range: std::ops::Range<usize>,
        context: &AnnotationContext,
    ) -> Vec<ComposedLine> {
        let sources = self.sources.read().expect("sources lock poisoned");

        // Collect annotations from all sources
        let mut store = AnnotationStore::new();
        for source in sources.iter() {
            let annotations = source.annotations(buffer_id, range.clone(), context);
            store.replace_source(SourceId::new(source.id()), annotations);
        }
        drop(sources);

        let presenters = self.presenters.read().expect("presenters lock poisoned");
        let config = self.config.read().expect("config lock poisoned");
        let composer = GutterComposer::new(&store, &presenters, &config);
        let presenter_ctx = PresenterContext::new(context.total_lines, context.cursor_line, false);
        let result = composer.compose_range(range.start, range.end, &presenter_ctx);
        drop(presenters);
        result
    }

    /// Calculate the total gutter width.
    ///
    /// # Arguments
    ///
    /// * `context` - Context with total lines (for line number width calculation)
    ///
    /// # Panics
    ///
    /// Panics if any internal lock is poisoned.
    #[must_use]
    pub fn total_width(&self, context: &AnnotationContext) -> usize {
        let presenters = self.presenters.read().expect("presenters lock poisoned");
        let config = self.config.read().expect("config lock poisoned");
        let store = AnnotationStore::new();
        let composer = GutterComposer::new(&store, &presenters, &config);
        let presenter_ctx = PresenterContext::new(context.total_lines, context.cursor_line, false);
        let result = composer.total_width(&presenter_ctx);
        drop(presenters);
        result
    }

    /// Check if any sources have annotations for the buffer.
    ///
    /// # Panics
    ///
    /// Panics if the internal lock is poisoned.
    #[must_use]
    pub fn has_annotations(&self, buffer_id: BufferId) -> bool {
        let sources = self.sources.read().expect("sources lock poisoned");
        sources.iter().any(|s| s.has_annotations(buffer_id))
    }

    /// Get the number of registered sources.
    ///
    /// # Panics
    ///
    /// Panics if the internal lock is poisoned.
    #[must_use]
    pub fn source_count(&self) -> usize {
        self.sources.read().expect("sources lock poisoned").len()
    }

    /// Get the number of registered presenters.
    ///
    /// # Panics
    ///
    /// Panics if the internal lock is poisoned.
    #[must_use]
    pub fn presenter_count(&self) -> usize {
        self.presenters
            .read()
            .expect("presenters lock poisoned")
            .len()
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
