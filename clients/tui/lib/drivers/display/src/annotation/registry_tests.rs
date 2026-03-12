use {
    super::*,
    crate::{annotation::Annotation, highlight::Style},
};

use super::super::presenter::{ColumnWidth, PresentedOutput, PresenterContext};

// Mock presenters for testing
struct ExactPresenter {
    id: &'static str,
    kind: String,
}

impl ExactPresenter {
    fn new(id: &'static str, kind: &str) -> Self {
        Self {
            id,
            kind: kind.to_string(),
        }
    }
}

impl AnnotationPresenter for ExactPresenter {
    fn id(&self) -> &'static str {
        self.id
    }

    fn handles(&self) -> KindPattern {
        KindPattern::exact(&self.kind)
    }

    fn present(&self, _: &Annotation, _: &PresenterContext) -> PresentedOutput {
        PresentedOutput::cell('E', Style::default())
    }

    fn column_width(&self, _: &PresenterContext) -> ColumnWidth {
        ColumnWidth::fixed(1)
    }
}

struct PrefixPresenter {
    id: &'static str,
    prefix: String,
}

impl PrefixPresenter {
    fn new(id: &'static str, prefix: &str) -> Self {
        Self {
            id,
            prefix: prefix.to_string(),
        }
    }
}

impl AnnotationPresenter for PrefixPresenter {
    fn id(&self) -> &'static str {
        self.id
    }

    fn handles(&self) -> KindPattern {
        KindPattern::prefix(&self.prefix)
    }

    fn present(&self, _: &Annotation, _: &PresenterContext) -> PresentedOutput {
        PresentedOutput::cell('P', Style::default())
    }

    fn column_width(&self, _: &PresenterContext) -> ColumnWidth {
        ColumnWidth::fixed(1)
    }
}

struct CatchAllPresenter;

impl AnnotationPresenter for CatchAllPresenter {
    fn id(&self) -> &'static str {
        "catch-all"
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

#[test]
fn test_registry_new() {
    let registry = PresenterRegistry::new();
    assert!(registry.is_empty());
    assert_eq!(registry.len(), 0);
}

#[test]
fn test_registry_register() {
    let mut registry = PresenterRegistry::new();
    registry.register(Arc::new(ExactPresenter::new("test", "line_number")));
    assert_eq!(registry.len(), 1);
    assert!(!registry.is_empty());
}

#[test]
fn test_registry_find_exact() {
    let mut registry = PresenterRegistry::new();
    registry.register(Arc::new(ExactPresenter::new("line", "line_number")));

    let kind = AnnotationKind::new("line_number");
    let presenter = registry.find(&kind);
    assert!(presenter.is_some());
    assert_eq!(presenter.unwrap().id(), "line");
}

#[test]
fn test_registry_find_prefix() {
    let mut registry = PresenterRegistry::new();
    registry.register(Arc::new(PrefixPresenter::new("diag", "diagnostic")));

    let kind = AnnotationKind::new("diagnostic.error");
    let presenter = registry.find(&kind);
    assert!(presenter.is_some());
    assert_eq!(presenter.unwrap().id(), "diag");
}

#[test]
fn test_registry_find_not_found() {
    let mut registry = PresenterRegistry::new();
    registry.register(Arc::new(ExactPresenter::new("line", "line_number")));

    let kind = AnnotationKind::new("diagnostic.error");
    let presenter = registry.find(&kind);
    assert!(presenter.is_none());
}

#[test]
fn test_registry_first_match_wins() {
    let mut registry = PresenterRegistry::new();

    // Register exact match first
    registry.register(Arc::new(ExactPresenter::new("exact", "diagnostic.error")));
    // Then prefix match
    registry.register(Arc::new(PrefixPresenter::new("prefix", "diagnostic")));
    // Then catch-all
    registry.register(Arc::new(CatchAllPresenter));

    // Exact should win for diagnostic.error
    let kind = AnnotationKind::new("diagnostic.error");
    let presenter = registry.find(&kind).unwrap();
    assert_eq!(presenter.id(), "exact");

    // Prefix should win for diagnostic.warning (no exact match)
    let kind = AnnotationKind::new("diagnostic.warning");
    let presenter = registry.find(&kind).unwrap();
    assert_eq!(presenter.id(), "prefix");

    // Catch-all should win for unknown kinds
    let kind = AnnotationKind::new("unknown");
    let presenter = registry.find(&kind).unwrap();
    assert_eq!(presenter.id(), "catch-all");
}

#[test]
fn test_registry_find_by_id() {
    let mut registry = PresenterRegistry::new();
    registry.register(Arc::new(ExactPresenter::new("test1", "kind1")));
    registry.register(Arc::new(ExactPresenter::new("test2", "kind2")));

    let presenter = registry.find_by_id("test1");
    assert!(presenter.is_some());
    assert_eq!(presenter.unwrap().id(), "test1");

    let presenter = registry.find_by_id("nonexistent");
    assert!(presenter.is_none());
}

#[test]
fn test_registry_has_presenter() {
    let mut registry = PresenterRegistry::new();
    registry.register(Arc::new(ExactPresenter::new("line", "line_number")));

    assert!(registry.has_presenter(&AnnotationKind::new("line_number")));
    assert!(!registry.has_presenter(&AnnotationKind::new("diagnostic")));
}

#[test]
fn test_registry_presenter_ids() {
    let mut registry = PresenterRegistry::new();
    registry.register(Arc::new(ExactPresenter::new("a", "kind_a")));
    registry.register(Arc::new(ExactPresenter::new("b", "kind_b")));

    let ids: Vec<_> = registry.presenter_ids().collect();
    assert_eq!(ids, vec!["a", "b"]);
}

#[test]
fn test_registry_unregister() {
    let mut registry = PresenterRegistry::new();
    registry.register(Arc::new(ExactPresenter::new("a", "kind_a")));
    registry.register(Arc::new(ExactPresenter::new("b", "kind_b")));

    assert!(registry.unregister("a"));
    assert_eq!(registry.len(), 1);
    assert!(registry.find_by_id("a").is_none());
    assert!(registry.find_by_id("b").is_some());
}

#[test]
fn test_registry_unregister_nonexistent() {
    let mut registry = PresenterRegistry::new();
    assert!(!registry.unregister("nonexistent"));
}

#[test]
fn test_registry_clear() {
    let mut registry = PresenterRegistry::new();
    registry.register(Arc::new(ExactPresenter::new("a", "kind_a")));
    registry.register(Arc::new(ExactPresenter::new("b", "kind_b")));

    registry.clear();
    assert!(registry.is_empty());
}

#[test]
fn test_registry_debug() {
    let mut registry = PresenterRegistry::new();
    registry.register(Arc::new(ExactPresenter::new("test", "kind")));

    let debug = format!("{registry:?}");
    assert!(debug.contains("PresenterRegistry"));
    assert!(debug.contains("test"));
}

#[test]
fn test_registry_is_send_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<PresenterRegistry>();
}

#[test]
fn test_exact_presenter_present_and_column_width() {
    let presenter = ExactPresenter::new("test", "line_number");
    let annotation = Annotation::line_number(0, 1);
    let ctx = PresenterContext {
        total_lines: 10,
        cursor_line: 0,
        is_cursor_line: false,
    };

    let output = presenter.present(&annotation, &ctx);
    assert_eq!(output.width(), 1);

    let width = presenter.column_width(&ctx);
    assert_eq!(width, ColumnWidth::fixed(1));
}

#[test]
fn test_prefix_presenter_present_and_column_width() {
    let presenter = PrefixPresenter::new("diag", "diagnostic");
    let annotation = Annotation::new(
        crate::annotation::AnnotationKind::new("diagnostic.error"),
        crate::annotation::AnnotationTarget::Line(0),
        50,
        crate::annotation::AnnotationPayload::Severity(0),
    );
    let ctx = PresenterContext {
        total_lines: 10,
        cursor_line: 0,
        is_cursor_line: false,
    };

    let output = presenter.present(&annotation, &ctx);
    assert_eq!(output.width(), 1);

    let width = presenter.column_width(&ctx);
    assert_eq!(width, ColumnWidth::fixed(1));
}

#[test]
fn test_catch_all_presenter_present_and_column_width() {
    let presenter = CatchAllPresenter;
    let annotation = Annotation::line_number(0, 1);
    let ctx = PresenterContext {
        total_lines: 10,
        cursor_line: 0,
        is_cursor_line: false,
    };

    let output = presenter.present(&annotation, &ctx);
    assert_eq!(output.width(), 1);

    let width = presenter.column_width(&ctx);
    assert_eq!(width, ColumnWidth::fixed(1));
}
