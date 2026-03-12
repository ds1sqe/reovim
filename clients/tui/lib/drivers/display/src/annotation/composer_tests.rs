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
