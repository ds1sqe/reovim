use reovim_arch::Color;

use {
    super::*,
    crate::annotation::{
        Annotation, AnnotationKind, AnnotationPayload, AnnotationTarget, PresenterContext,
    },
};

fn ctx() -> PresenterContext {
    PresenterContext {
        total_lines: 100,
        cursor_line: 0,
        is_cursor_line: false,
    }
}

fn make_diagnostic(severity: u8) -> Annotation {
    Annotation {
        kind: AnnotationKind::new("diagnostic.error"),
        target: AnnotationTarget::Line(0),
        priority: 50,
        payload: AnnotationPayload::Severity(severity),
    }
}

#[test]
fn presenter_id() {
    let p = DiagnosticPresenter::new();
    assert_eq!(p.id(), "diagnostics");
}

#[test]
fn handles_diagnostic_prefix() {
    let p = DiagnosticPresenter::new();
    assert!(p.can_handle(&AnnotationKind::new("diagnostic.error")));
    assert!(p.can_handle(&AnnotationKind::new("diagnostic.warning")));
    assert!(p.can_handle(&AnnotationKind::new("diagnostic.info")));
    assert!(p.can_handle(&AnnotationKind::new("diagnostic.hint")));
    assert!(!p.can_handle(&AnnotationKind::new("git.add")));
}

#[test]
fn present_error() {
    let p = DiagnosticPresenter::new();
    let ann = make_diagnostic(0);
    let output = p.present(&ann, &ctx());
    match output {
        PresentedOutput::Cell(cell) => {
            assert_eq!(cell.char, ERROR_ICON);
            assert_eq!(cell.style.fg, Some(Color::Red));
        }
        _ => panic!("expected Cell output"),
    }
}

#[test]
fn present_warning() {
    let p = DiagnosticPresenter::new();
    let ann = make_diagnostic(1);
    let output = p.present(&ann, &ctx());
    match output {
        PresentedOutput::Cell(cell) => {
            assert_eq!(cell.char, WARNING_ICON);
            assert_eq!(cell.style.fg, Some(Color::Yellow));
        }
        _ => panic!("expected Cell output"),
    }
}

#[test]
fn present_info() {
    let p = DiagnosticPresenter::new();
    let ann = make_diagnostic(2);
    let output = p.present(&ann, &ctx());
    match output {
        PresentedOutput::Cell(cell) => {
            assert_eq!(cell.char, INFO_ICON);
            assert_eq!(cell.style.fg, Some(Color::Blue));
        }
        _ => panic!("expected Cell output"),
    }
}

#[test]
fn present_hint() {
    let p = DiagnosticPresenter::new();
    let ann = make_diagnostic(3);
    let output = p.present(&ann, &ctx());
    match output {
        PresentedOutput::Cell(cell) => {
            assert_eq!(cell.char, HINT_ICON);
            assert_eq!(cell.style.fg, Some(Color::Cyan));
        }
        _ => panic!("expected Cell output"),
    }
}

#[test]
fn present_unknown_severity_defaults_to_error() {
    let p = DiagnosticPresenter::new();
    let ann = make_diagnostic(255);
    let output = p.present(&ann, &ctx());
    match output {
        PresentedOutput::Cell(cell) => {
            assert_eq!(cell.char, ERROR_ICON);
        }
        _ => panic!("expected Cell output for unknown severity"),
    }
}

#[test]
fn present_non_severity_payload_hidden() {
    let p = DiagnosticPresenter::new();
    let ann = Annotation {
        kind: AnnotationKind::new("diagnostic.error"),
        target: AnnotationTarget::Line(0),
        priority: 50,
        payload: AnnotationPayload::None,
    };
    let output = p.present(&ann, &ctx());
    assert!(matches!(output, PresentedOutput::Hidden));
}

#[test]
fn column_width_is_one() {
    let p = DiagnosticPresenter::new();
    let width = p.column_width(&ctx());
    assert_eq!(width, ColumnWidth::fixed(1));
}

#[test]
fn default_impl() {
    fn assert_default<T: Default>(_: T) {}
    assert_default(DiagnosticPresenter);
}
