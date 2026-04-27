//! Gutter composer for orchestrating annotation rendering.
//!
//! The composer brings together sources, presenters, and configuration
//! to produce the final gutter output.
//!
//! # Architecture
//!
//! ```text
//! AnnotationStore (data) ──┐
//!                          │
//! PresenterRegistry ───────┼──▶ GutterComposer ──▶ Vec<GutterCell>
//!                          │
//! GutterConfig ────────────┘
//! ```
//!
//! The composer is stateless and deterministic - given the same inputs,
//! it always produces the same output.

use std::sync::Arc;

use crate::highlight::Style;

use super::{
    AnnotationKind, AnnotationStore,
    config::{ColumnConfig, GutterConfig},
    presenter::{AnnotationPresenter, GutterCell, PresentedOutput, PresenterContext},
    registry::PresenterRegistry,
};

/// Result of composing a single line's gutter.
#[derive(Debug, Clone)]
pub struct ComposedLine {
    /// Cells for this line's gutter.
    pub cells: Vec<GutterCell>,
    /// Total width in characters.
    pub width: usize,
}

impl ComposedLine {
    /// Create an empty composed line.
    ///
    /// Note: Cannot be const because `Vec::new()` is not const.
    #[must_use]
    #[allow(clippy::missing_const_for_fn)]
    pub fn empty() -> Self {
        Self {
            cells: Vec::new(),
            width: 0,
        }
    }

    /// Create a composed line with the given cells.
    #[must_use]
    pub fn new(cells: Vec<GutterCell>) -> Self {
        let width = cells.iter().map(GutterCell::width).sum();
        Self { cells, width }
    }
}

/// Gutter composer for rendering annotations.
///
/// The composer is the central orchestration component that:
/// 1. Queries the store for annotations
/// 2. Finds appropriate presenters
/// 3. Renders annotations to cells
/// 4. Arranges cells according to configuration
pub struct GutterComposer<'a> {
    /// Annotation store (data source).
    store: &'a AnnotationStore,
    /// Presenter registry (renderers).
    registry: &'a PresenterRegistry,
    /// Gutter configuration.
    config: &'a GutterConfig,
}

impl<'a> GutterComposer<'a> {
    /// Create a new composer.
    #[must_use]
    pub const fn new(
        store: &'a AnnotationStore,
        registry: &'a PresenterRegistry,
        config: &'a GutterConfig,
    ) -> Self {
        Self {
            store,
            registry,
            config,
        }
    }

    /// Calculate the total gutter width.
    ///
    /// This considers all columns, their visibility, and width settings.
    /// Columns are iterated in priority order (lower = further left).
    #[must_use]
    pub fn total_width(&self, ctx: &PresenterContext) -> usize {
        let mut total = 0;

        for col_config in self.sorted_columns() {
            // Check visibility
            let has_annotations = self.has_annotations_for_pattern(&col_config.pattern);
            if !col_config.visibility.should_show(has_annotations) {
                continue;
            }

            // Get width
            let width = col_config.width.map_or_else(
                || self.get_column_width(&col_config.pattern, ctx),
                |fixed| fixed as usize,
            );

            total += width;
        }

        // Add separator if configured
        if self.config.show_separator && total > 0 {
            total += 1;
        }

        total
    }

    /// Compose the gutter for a single line.
    ///
    /// Columns are rendered in priority order (lower = further left).
    #[must_use]
    pub fn compose_line(&self, line: usize, ctx: &PresenterContext) -> ComposedLine {
        if self.config.is_empty() {
            return ComposedLine::empty();
        }

        let mut cells = Vec::new();
        let annotations = self.store.query_line(line);

        for col_config in self.sorted_columns() {
            // Check visibility
            let has_annotations = self.has_annotations_for_pattern(&col_config.pattern);
            if !col_config.visibility.should_show(has_annotations) {
                continue;
            }

            // Get column width
            let col_width = col_config.width.map_or_else(
                || self.get_column_width(&col_config.pattern, ctx),
                |fixed| fixed as usize,
            );

            // Find annotation for this column (highest priority)
            let annotation = annotations
                .iter()
                .find(|a| col_config.pattern.matches(&a.kind));

            // Render the cell(s)
            let output =
                annotation.map_or(PresentedOutput::Hidden, |ann| self.render_annotation(ann, ctx));

            // Add cells with padding
            Self::add_cells_padded(&mut cells, output, col_width);
        }

        // Add separator if configured
        if self.config.show_separator && !cells.is_empty() {
            cells.push(GutterCell::new('│', Style::default()));
        }

        ComposedLine::new(cells)
    }

    /// Compose the gutter for a range of lines.
    #[must_use]
    pub fn compose_range(
        &self,
        start_line: usize,
        end_line: usize,
        ctx: &PresenterContext,
    ) -> Vec<ComposedLine> {
        (start_line..end_line)
            .map(|line| {
                let line_ctx = PresenterContext {
                    total_lines: ctx.total_lines,
                    cursor_line: ctx.cursor_line,
                    is_cursor_line: line == ctx.cursor_line,
                };
                self.compose_line(line, &line_ctx)
            })
            .collect()
    }

    /// Return columns sorted by priority (lower = further left).
    fn sorted_columns(&self) -> Vec<&ColumnConfig> {
        let mut cols: Vec<&ColumnConfig> = self.config.columns.iter().collect();
        cols.sort_by_key(|c| c.priority);
        cols
    }

    /// Check if any annotations exist for the given pattern.
    fn has_annotations_for_pattern(&self, _pattern: &super::presenter::KindPattern) -> bool {
        // Check if any source provides annotations matching this pattern
        // For efficiency, we just check if any layer has data
        // A more precise check would iterate annotations and match against pattern
        self.store
            .source_ids()
            .any(|source_id| self.store.layer(source_id).is_some_and(|l| !l.is_empty()))
    }

    /// Get the width for a column based on its pattern.
    fn get_column_width(
        &self,
        pattern: &super::presenter::KindPattern,
        ctx: &PresenterContext,
    ) -> usize {
        // Create a dummy kind to find the presenter
        let dummy_kind = Self::kind_for_pattern(pattern);
        self.registry.find(&dummy_kind).map_or(1, |presenter| {
            let width = presenter.column_width(ctx);
            width.resolve(width.max_width()) as usize
        })
    }

    /// Create a representative kind for a pattern (for presenter lookup).
    fn kind_for_pattern(pattern: &super::presenter::KindPattern) -> AnnotationKind {
        match pattern {
            super::presenter::KindPattern::Exact(name) => AnnotationKind::new(name.as_str()),
            super::presenter::KindPattern::Prefix(prefix) => {
                // Create a kind that will match this prefix
                AnnotationKind::new(prefix.as_str())
            }
            super::presenter::KindPattern::All => AnnotationKind::new("_any"),
        }
    }

    /// Render an annotation using the appropriate presenter.
    fn render_annotation(
        &self,
        annotation: &super::Annotation,
        ctx: &PresenterContext,
    ) -> PresentedOutput {
        self.registry
            .find(&annotation.kind)
            .map_or(PresentedOutput::Hidden, |presenter| presenter.present(annotation, ctx))
    }

    /// Add cells to output with padding to reach target width.
    fn add_cells_padded(cells: &mut Vec<GutterCell>, output: PresentedOutput, width: usize) {
        let output_cells = output.into_cells();
        let output_width: usize = output_cells.iter().map(GutterCell::width).sum();

        // Right-align: add padding first
        if output_width < width {
            for _ in 0..(width - output_width) {
                cells.push(GutterCell::space());
            }
        }

        // Add the actual cells (possibly truncated)
        let mut remaining = width;
        for cell in output_cells {
            let cell_width = cell.width();
            if cell_width <= remaining {
                cells.push(cell);
                remaining -= cell_width;
            } else {
                break;
            }
        }
    }
}

/// Builder for creating a composer with all dependencies.
pub struct ComposerBuilder {
    config: GutterConfig,
    presenters: Vec<Arc<dyn AnnotationPresenter>>,
}

impl ComposerBuilder {
    /// Create a new builder with default configuration.
    #[must_use]
    pub fn new() -> Self {
        Self {
            config: GutterConfig::default_line_numbers(),
            presenters: Vec::new(),
        }
    }

    /// Set the gutter configuration.
    #[must_use]
    pub fn config(mut self, config: GutterConfig) -> Self {
        self.config = config;
        self
    }

    /// Add a presenter.
    #[must_use]
    pub fn presenter(mut self, presenter: Arc<dyn AnnotationPresenter>) -> Self {
        self.presenters.push(presenter);
        self
    }

    /// Build the registry and config.
    ///
    /// Returns a tuple of (registry, config) that can be used with `GutterComposer::new()`.
    #[must_use]
    pub fn build(self) -> (PresenterRegistry, GutterConfig) {
        let mut registry = PresenterRegistry::new();
        for presenter in self.presenters {
            registry.register(presenter);
        }
        (registry, self.config)
    }
}

impl Default for ComposerBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
#[path = "composer_tests.rs"]
mod tests;
