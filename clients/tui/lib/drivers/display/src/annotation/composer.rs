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
    config::GutterConfig,
    presenter::{AnnotationPresenter, GutterCell, PresentedOutput, PresenterContext},
    registry::PresenterRegistry,
    store::AnnotationStore,
    types::AnnotationKind,
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
    #[must_use]
    pub fn total_width(&self, ctx: &PresenterContext) -> usize {
        let mut total = 0;

        for col_config in &self.config.columns {
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
    #[must_use]
    pub fn compose_line(&self, line: usize, ctx: &PresenterContext) -> ComposedLine {
        if self.config.is_empty() {
            return ComposedLine::empty();
        }

        let mut cells = Vec::new();
        let annotations = self.store.query_line(line);

        for col_config in &self.config.columns {
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
        annotation: &super::types::Annotation,
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
mod tests {
    use {
        super::*,
        crate::annotation::{
            Annotation, ColumnConfig, ColumnWidth, KindPattern, SourceId, VisibilityMode,
        },
    };

    // Mock presenter for testing
    struct MockPresenter {
        id: &'static str,
        pattern: KindPattern,
        char: char,
        width: ColumnWidth,
    }

    impl MockPresenter {
        fn line_number() -> Self {
            Self {
                id: "line_number",
                pattern: KindPattern::exact("line_number"),
                char: '#',
                width: ColumnWidth::dynamic(2, 6),
            }
        }

        fn sign() -> Self {
            Self {
                id: "sign",
                pattern: KindPattern::prefix("sign"),
                char: 'S',
                width: ColumnWidth::fixed(2),
            }
        }
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    impl AnnotationPresenter for MockPresenter {
        fn id(&self) -> &'static str {
            self.id
        }

        fn handles(&self) -> KindPattern {
            self.pattern.clone()
        }

        #[allow(clippy::option_if_let_else)]
        fn present(&self, annotation: &Annotation, _ctx: &PresenterContext) -> PresentedOutput {
            if let Some(n) = annotation.payload.as_number() {
                PresentedOutput::text(&n.to_string(), &Style::default())
            } else {
                PresentedOutput::cell(self.char, Style::default())
            }
        }

        #[allow(
            clippy::cast_precision_loss,
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss
        )]
        fn column_width(&self, ctx: &PresenterContext) -> ColumnWidth {
            if let ColumnWidth::Dynamic { min, max } = self.width {
                // Calculate digits needed
                let digits = if ctx.total_lines == 0 {
                    1
                } else {
                    (ctx.total_lines as f64).log10().floor() as u16 + 1
                };
                ColumnWidth::dynamic(min, digits.max(min).min(max))
            } else {
                self.width
            }
        }
    }

    fn setup_test() -> (AnnotationStore, PresenterRegistry, GutterConfig) {
        let mut store = AnnotationStore::new();
        store.replace_source(
            SourceId::new("line_number"),
            vec![
                Annotation::line_number(0, 1),
                Annotation::line_number(1, 2),
                Annotation::line_number(2, 3),
            ],
        );

        let mut registry = PresenterRegistry::new();
        registry.register(Arc::new(MockPresenter::line_number()));

        let config = GutterConfig::default_line_numbers();

        (store, registry, config)
    }

    #[test]
    fn test_composed_line_empty() {
        let line = ComposedLine::empty();
        assert!(line.cells.is_empty());
        assert_eq!(line.width, 0);
    }

    #[test]
    fn test_composed_line_new() {
        let cells = vec![
            GutterCell::new('1', Style::default()),
            GutterCell::new('2', Style::default()),
        ];
        let line = ComposedLine::new(cells);
        assert_eq!(line.width, 2);
    }

    #[test]
    fn test_composer_new() {
        let (store, registry, config) = setup_test();
        let _composer = GutterComposer::new(&store, &registry, &config);
    }

    #[test]
    fn test_composer_total_width() {
        let (store, registry, config) = setup_test();
        let composer = GutterComposer::new(&store, &registry, &config);
        let ctx = PresenterContext::new(100, 0, false);

        let width = composer.total_width(&ctx);
        // Should be column width + separator
        assert!(width > 0);
    }

    #[test]
    fn test_composer_compose_line() {
        let (store, registry, config) = setup_test();
        let composer = GutterComposer::new(&store, &registry, &config);
        let ctx = PresenterContext::new(100, 0, true);

        let line = composer.compose_line(0, &ctx);
        assert!(!line.cells.is_empty());
    }

    #[test]
    fn test_composer_compose_range() {
        let (store, registry, config) = setup_test();
        let composer = GutterComposer::new(&store, &registry, &config);
        let ctx = PresenterContext::new(100, 0, false);

        let lines = composer.compose_range(0, 3, &ctx);
        assert_eq!(lines.len(), 3);
    }

    #[test]
    fn test_composer_empty_config() {
        let (store, registry, _) = setup_test();
        let config = GutterConfig::none();
        let composer = GutterComposer::new(&store, &registry, &config);
        let ctx = PresenterContext::new(100, 0, false);

        let line = composer.compose_line(0, &ctx);
        assert!(line.cells.is_empty());
    }

    #[test]
    fn test_composer_visibility_auto_no_annotations() {
        let store = AnnotationStore::new(); // Empty store
        let mut registry = PresenterRegistry::new();
        registry.register(Arc::new(MockPresenter::sign()));

        let config = GutterConfig::new(vec![
            ColumnConfig::new(KindPattern::prefix("sign")).visibility(VisibilityMode::Auto),
        ]);

        let composer = GutterComposer::new(&store, &registry, &config);
        let ctx = PresenterContext::new(100, 0, false);

        // Should show nothing because no annotations exist
        let width = composer.total_width(&ctx);
        assert_eq!(width, 0);
    }

    #[test]
    fn test_composer_visibility_always() {
        let store = AnnotationStore::new(); // Empty store
        let mut registry = PresenterRegistry::new();
        registry.register(Arc::new(MockPresenter::line_number()));

        let config = GutterConfig::new(vec![
            ColumnConfig::new(KindPattern::exact("line_number"))
                .visibility(VisibilityMode::Always)
                .width(4),
        ]);

        let composer = GutterComposer::new(&store, &registry, &config);
        let ctx = PresenterContext::new(100, 0, false);

        // Should show column even without annotations
        let width = composer.total_width(&ctx);
        assert_eq!(width, 5); // 4 + separator
    }

    #[test]
    fn test_composer_builder() {
        let builder = ComposerBuilder::new()
            .config(GutterConfig::full())
            .presenter(Arc::new(MockPresenter::line_number()))
            .presenter(Arc::new(MockPresenter::sign()));

        let (registry, config) = builder.build();
        assert_eq!(registry.len(), 2);
        assert_eq!(config.column_count(), 5);
    }

    #[test]
    fn test_composer_builder_default() {
        let builder = ComposerBuilder::default();
        let (registry, config) = builder.build();
        assert!(registry.is_empty());
        assert_eq!(config.column_count(), 1);
    }

    // ========================================================================
    // Additional coverage tests
    // ========================================================================

    #[test]
    fn test_compose_line_skips_never_visible_columns() {
        // Covers line 137: `continue` when visibility is Never
        let mut store = AnnotationStore::new();
        store.replace_source(SourceId::new("line_number"), vec![Annotation::line_number(0, 1)]);

        let mut registry = PresenterRegistry::new();
        registry.register(Arc::new(MockPresenter::line_number()));

        let config = GutterConfig::new(vec![
            ColumnConfig::new(KindPattern::exact("line_number")).visibility(VisibilityMode::Never),
        ]);

        let composer = GutterComposer::new(&store, &registry, &config);
        let ctx = PresenterContext::new(10, 0, true);
        let line = composer.compose_line(0, &ctx);
        // Column with Never visibility should be skipped
        assert!(line.cells.is_empty());
    }

    #[test]
    fn test_compose_line_fixed_width_column() {
        // Covers line 143: fixed width branch in map_or_else
        let mut store = AnnotationStore::new();
        store.replace_source(SourceId::new("line_number"), vec![Annotation::line_number(0, 1)]);

        let mut registry = PresenterRegistry::new();
        registry.register(Arc::new(MockPresenter::line_number()));

        let config = GutterConfig::new(vec![
            ColumnConfig::new(KindPattern::exact("line_number"))
                .visibility(VisibilityMode::Always)
                .width(6),
        ]);

        let composer = GutterComposer::new(&store, &registry, &config);
        let ctx = PresenterContext::new(10, 0, true);
        let line = composer.compose_line(0, &ctx);
        assert!(!line.cells.is_empty());
    }

    #[test]
    fn test_compose_line_has_annotations_for_pattern_true() {
        // Covers line 197: has_annotations_for_pattern returning true
        // The sign column is Auto, store has annotations -> shown
        let mut store = AnnotationStore::new();
        store.replace_source(SourceId::new("line_number"), vec![Annotation::line_number(0, 1)]);

        let mut registry = PresenterRegistry::new();
        registry.register(Arc::new(MockPresenter::line_number()));
        registry.register(Arc::new(MockPresenter::sign()));

        let config = GutterConfig::new(vec![
            ColumnConfig::new(KindPattern::prefix("sign"))
                .visibility(VisibilityMode::Auto)
                .width(2),
            ColumnConfig::new(KindPattern::exact("line_number")).visibility(VisibilityMode::Always),
        ]);

        let composer = GutterComposer::new(&store, &registry, &config);
        let ctx = PresenterContext::new(10, 0, true);
        let line = composer.compose_line(0, &ctx);
        assert!(!line.cells.is_empty());
    }

    struct CatchAllPresenter;
    #[cfg_attr(coverage_nightly, coverage(off))]
    impl AnnotationPresenter for CatchAllPresenter {
        fn id(&self) -> &'static str {
            "catch_all"
        }
        fn handles(&self) -> KindPattern {
            KindPattern::All
        }
        fn present(&self, _: &Annotation, _: &PresenterContext) -> PresentedOutput {
            PresentedOutput::cell('*', Style::default())
        }
        fn column_width(&self, _: &PresenterContext) -> ColumnWidth {
            ColumnWidth::fixed(1)
        }
    }

    struct WideOutputPresenter;
    #[cfg_attr(coverage_nightly, coverage(off))]
    impl AnnotationPresenter for WideOutputPresenter {
        fn id(&self) -> &'static str {
            "wide"
        }
        fn handles(&self) -> KindPattern {
            KindPattern::exact("test")
        }
        fn present(&self, annotation: &Annotation, _: &PresenterContext) -> PresentedOutput {
            annotation
                .payload
                .as_number()
                .map_or(PresentedOutput::Hidden, |n| {
                    PresentedOutput::text(&n.to_string(), &Style::default())
                })
        }
        fn column_width(&self, _: &PresenterContext) -> ColumnWidth {
            ColumnWidth::fixed(3)
        }
    }

    #[test]
    fn test_total_width_with_prefix_and_all_patterns() {
        // Covers lines 220, 222, 224: kind_for_pattern Prefix and All branches
        let store = AnnotationStore::new();
        let mut registry = PresenterRegistry::new();
        registry.register(Arc::new(MockPresenter::sign()));
        registry.register(Arc::new(CatchAllPresenter));

        let config = GutterConfig::new(vec![
            ColumnConfig::new(KindPattern::prefix("sign")).visibility(VisibilityMode::Always),
            ColumnConfig::new(KindPattern::All)
                .visibility(VisibilityMode::Always)
                .width(1),
        ]);

        let composer = GutterComposer::new(&store, &registry, &config);
        let ctx = PresenterContext::new(10, 0, false);
        let width = composer.total_width(&ctx);
        assert!(width > 0);
    }

    #[test]
    fn test_has_annotations_for_pattern_returns_true() {
        // Covers line 197: `return true;` when store has non-empty layers
        let mut store = AnnotationStore::new();
        store.replace_source(SourceId::new("test_source"), vec![Annotation::line_number(0, 1)]);

        let mut registry = PresenterRegistry::new();
        registry.register(Arc::new(CatchAllPresenter));

        // Auto visibility with non-fixed width -> has_annotations_for_pattern is called
        // AND get_column_width with KindPattern::All is called (covers line 224)
        let config = GutterConfig::new(vec![
            ColumnConfig::new(KindPattern::All).visibility(VisibilityMode::Auto),
        ]);

        let composer = GutterComposer::new(&store, &registry, &config);
        let ctx = PresenterContext::new(10, 0, false);

        // Since store has data, has_annotations_for_pattern returns true (line 197)
        // And get_column_width calls kind_for_pattern with KindPattern::All (line 224)
        let width = composer.total_width(&ctx);
        assert!(width > 0);
    }

    #[test]
    fn test_total_width_no_separator_with_content() {
        // Line 116: show_separator=false but total > 0
        // The separator width (+1) should NOT be added
        let store = AnnotationStore::new();
        let mut registry = PresenterRegistry::new();
        registry.register(Arc::new(MockPresenter::sign()));

        let config = GutterConfig::new(vec![
            ColumnConfig::new(KindPattern::exact("sign"))
                .visibility(VisibilityMode::Always)
                .width(2),
        ])
        .with_separator(false);

        let composer = GutterComposer::new(&store, &registry, &config);
        let ctx = PresenterContext::new(10, 0, false);
        let width = composer.total_width(&ctx);
        // Should be 2 (column width) without +1 separator
        assert_eq!(width, 2);
    }

    #[test]
    fn test_compose_line_no_separator_with_cells() {
        // Line 160: show_separator=false and cells are non-empty
        // The separator cell should NOT be appended
        let mut store = AnnotationStore::new();
        store.replace_source(
            SourceId::new("test"),
            vec![Annotation::new(
                AnnotationKind::new("sign"),
                crate::annotation::AnnotationTarget::Line(0),
                0,
                crate::annotation::AnnotationPayload::Text("!".into()),
            )],
        );

        let mut registry = PresenterRegistry::new();
        registry.register(Arc::new(MockPresenter::sign()));

        let config = GutterConfig::new(vec![
            ColumnConfig::new(KindPattern::exact("sign"))
                .visibility(VisibilityMode::Always)
                .width(2),
        ])
        .with_separator(false);

        let composer = GutterComposer::new(&store, &registry, &config);
        let ctx = PresenterContext::new(10, 0, true);
        let line = composer.compose_line(0, &ctx);
        // No separator cell should be present
        assert!(!line.cells.iter().any(|c| c.char == '\u{2502}')); // '│'
    }

    #[test]
    fn test_add_cells_padded_output_wider_than_column() {
        // Covers lines 250 (no padding) and 260 (break on truncation)
        let mut store = AnnotationStore::new();
        store.replace_source(
            SourceId::new("test"),
            vec![Annotation::new(
                AnnotationKind::new("test"),
                crate::annotation::AnnotationTarget::Line(0),
                0,
                crate::annotation::AnnotationPayload::Number(12345),
            )],
        );

        let mut registry = PresenterRegistry::new();
        registry.register(Arc::new(WideOutputPresenter));

        let config = GutterConfig::new(vec![
            ColumnConfig::new(KindPattern::exact("test"))
                .visibility(VisibilityMode::Always)
                .width(3),
        ]);

        let composer = GutterComposer::new(&store, &registry, &config);
        let ctx = PresenterContext::new(10, 0, true);
        let line = composer.compose_line(0, &ctx);
        // "12345" is 5 chars wide, column is 3 wide
        // No padding (output >= width), then truncation after 3 chars
        assert!(!line.cells.is_empty());
        // Width should be 3 (truncated) + 1 (separator)
        assert_eq!(line.width, 4);
    }
}
