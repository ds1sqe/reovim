use {
    crate::annotation::{
        Annotation, AnnotationKind, AnnotationPayload, AnnotationPresenter, AnnotationTarget,
        ColumnWidth, KindPattern, PresenterContext,
    },
    reovim_arch::Color,
};

use super::*;

fn ctx() -> PresenterContext {
    PresenterContext::new(100, 0, false)
}

fn annotation(kind: &str) -> Annotation {
    Annotation {
        kind: AnnotationKind::new(kind),
        target: AnnotationTarget::Line(0),
        priority: 10,
        payload: AnnotationPayload::None,
    }
}

#[test]
fn test_id() {
    let p = GitSignsPresenter::new();
    assert_eq!(p.id(), "git-signs");
}

#[test]
fn test_handles_git_prefix() {
    let p = GitSignsPresenter::new();
    assert_eq!(p.handles(), KindPattern::prefix("git"));
}

#[test]
fn test_can_handle_git_kinds() {
    let p = GitSignsPresenter::new();
    assert!(p.can_handle(&AnnotationKind::new("git.add")));
    assert!(p.can_handle(&AnnotationKind::new("git.change")));
    assert!(p.can_handle(&AnnotationKind::new("git.delete")));
    assert!(!p.can_handle(&AnnotationKind::new("diagnostic.error")));
    assert!(!p.can_handle(&AnnotationKind::new("line_number")));
}

#[test]
fn test_present_add() {
    let p = GitSignsPresenter::new();
    let result = p.present(&annotation("git.add"), &ctx());
    match result {
        PresentedOutput::Cell(cell) => {
            assert_eq!(cell.char, '+');
            assert_eq!(cell.style.fg, Some(Color::Green));
        }
        _ => panic!("expected Cell"),
    }
}

#[test]
fn test_present_change() {
    let p = GitSignsPresenter::new();
    let result = p.present(&annotation("git.change"), &ctx());
    match result {
        PresentedOutput::Cell(cell) => {
            assert_eq!(cell.char, '~');
            assert_eq!(cell.style.fg, Some(Color::Yellow));
        }
        _ => panic!("expected Cell"),
    }
}

#[test]
fn test_present_delete() {
    let p = GitSignsPresenter::new();
    let result = p.present(&annotation("git.delete"), &ctx());
    match result {
        PresentedOutput::Cell(cell) => {
            assert_eq!(cell.char, '_');
            assert_eq!(cell.style.fg, Some(Color::Red));
        }
        _ => panic!("expected Cell"),
    }
}

#[test]
fn test_present_unknown_git_kind() {
    let p = GitSignsPresenter::new();
    let result = p.present(&annotation("git.unknown"), &ctx());
    assert!(result.is_hidden());
}

#[test]
fn test_column_width() {
    let p = GitSignsPresenter::new();
    assert_eq!(p.column_width(&ctx()), ColumnWidth::fixed(1));
}

#[test]
fn test_default() {
    let p = GitSignsPresenter;
    assert_eq!(p.id(), "git-signs");
}
