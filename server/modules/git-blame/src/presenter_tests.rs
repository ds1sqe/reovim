use reovim_driver_display::{
    Color,
    annotation::{
        Annotation, AnnotationKind, AnnotationPayload, AnnotationPresenter, AnnotationTarget,
        ColumnWidth, KindPattern, PresenterContext,
    },
};

use super::*;

fn ctx() -> PresenterContext {
    PresenterContext::new(100, 0, false)
}

#[test]
fn test_id() {
    let p = BlamePresenter::new();
    assert_eq!(p.id(), "blame");
}

#[test]
fn test_handles_blame_prefix() {
    let p = BlamePresenter::new();
    assert_eq!(p.handles(), KindPattern::prefix("blame"));
}

#[test]
fn test_can_handle() {
    let p = BlamePresenter::new();
    assert!(p.can_handle(&AnnotationKind::new("blame.info")));
    assert!(!p.can_handle(&AnnotationKind::new("git.add")));
}

#[test]
fn test_present_with_text() {
    let p = BlamePresenter::new();
    let annotation = Annotation {
        kind: AnnotationKind::new("blame.info"),
        target: AnnotationTarget::Line(0),
        priority: 5,
        payload: AnnotationPayload::text("abc1234 Alice feat: add"),
    };
    let result = p.present(&annotation, &ctx());
    let cells = result.into_cells();
    assert!(!cells.is_empty());
    assert_eq!(cells[0].char, 'a');
    assert_eq!(cells[0].style.fg, Some(Color::DarkGrey));
}

#[test]
fn test_present_no_text() {
    let p = BlamePresenter::new();
    let annotation = Annotation {
        kind: AnnotationKind::new("blame.info"),
        target: AnnotationTarget::Line(0),
        priority: 5,
        payload: AnnotationPayload::None,
    };
    let result = p.present(&annotation, &ctx());
    assert!(result.is_hidden());
}

#[test]
fn test_column_width_dynamic() {
    let p = BlamePresenter::new();
    let width = p.column_width(&ctx());
    assert_eq!(width, ColumnWidth::dynamic(0, 60));
}

#[test]
fn test_default() {
    let p = BlamePresenter;
    assert_eq!(p.id(), "blame");
}
