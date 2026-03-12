use super::*;

#[test]
fn test_span_affects_line() {
    let span = Span::new(5, 0, 10, 20);
    assert!(!span.affects_line(4));
    assert!(span.affects_line(5));
    assert!(span.affects_line(7));
    assert!(span.affects_line(10));
    assert!(!span.affects_line(11));
}

#[test]
fn test_span_single_line() {
    let single = Span::line(5, 0, 10);
    assert!(single.is_single_line());

    let multi = Span::new(5, 0, 6, 10);
    assert!(!multi.is_single_line());
}

#[test]
fn test_decoration_group_ordering() {
    assert!(DecorationGroup::Language < DecorationGroup::Syntax);
    assert!(DecorationGroup::Syntax < DecorationGroup::Search);
    assert!(DecorationGroup::Search < DecorationGroup::Diagnostic);
    assert!(DecorationGroup::Diagnostic < DecorationGroup::Visual);
}

#[test]
fn test_decoration_affects_line() {
    let conceal = Decoration::conceal(Span::line(5, 0, 10), "test", None);
    assert!(!conceal.affects_line(4));
    assert!(conceal.affects_line(5));
    assert!(!conceal.affects_line(6));

    let line_bg = Decoration::line_background(5, 10, Style::default());
    assert!(!line_bg.affects_line(4));
    assert!(line_bg.affects_line(5));
    assert!(line_bg.affects_line(7));
    assert!(line_bg.affects_line(10));
    assert!(!line_bg.affects_line(11));
}

#[test]
fn test_decoration_type_checks() {
    let conceal = Decoration::conceal(Span::line(0, 0, 5), "x", None);
    assert!(conceal.is_conceal());
    assert!(!conceal.is_line_background());

    let line_bg = Decoration::line_background(0, 0, Style::default());
    assert!(line_bg.is_line_background());
    assert!(!line_bg.is_conceal());

    let hide = Decoration::hide(Span::line(0, 0, 5));
    assert!(hide.is_hide());

    let inline = Decoration::inline_style(Span::line(0, 0, 5), Style::default());
    assert!(inline.is_inline_style());
}

#[test]
fn test_decoration_group_priority() {
    assert_eq!(DecorationGroup::Language.priority(), 0);
    assert_eq!(DecorationGroup::Syntax.priority(), 10);
    assert_eq!(DecorationGroup::Search.priority(), 20);
    assert_eq!(DecorationGroup::Diagnostic.priority(), 30);
    assert_eq!(DecorationGroup::Visual.priority(), 40);
}

#[test]
fn test_decoration_start_line() {
    let conceal = Decoration::conceal(Span::new(5, 0, 10, 20), "x", None);
    assert_eq!(conceal.start_line(), 5);

    let hide = Decoration::hide(Span::new(3, 0, 7, 10));
    assert_eq!(hide.start_line(), 3);

    let inline = Decoration::inline_style(Span::new(8, 5, 12, 15), Style::default());
    assert_eq!(inline.start_line(), 8);

    let line_bg = Decoration::line_background(15, 20, Style::default());
    assert_eq!(line_bg.start_line(), 15);
}

#[test]
fn test_decoration_end_line() {
    let conceal = Decoration::conceal(Span::new(5, 0, 10, 20), "x", None);
    assert_eq!(conceal.end_line(), 10);

    let hide = Decoration::hide(Span::new(3, 0, 7, 10));
    assert_eq!(hide.end_line(), 7);

    let inline = Decoration::inline_style(Span::new(8, 5, 12, 15), Style::default());
    assert_eq!(inline.end_line(), 12);

    let line_bg = Decoration::line_background(15, 20, Style::default());
    assert_eq!(line_bg.end_line(), 20);
}

#[test]
fn test_decoration_group_default() {
    let default_group = DecorationGroup::default();
    assert_eq!(default_group, DecorationGroup::Language);
}

#[test]
fn test_decoration_line_background_affects_line() {
    let line_bg = Decoration::line_background(5, 10, Style::default());
    assert!(!line_bg.affects_line(4));
    assert!(line_bg.affects_line(5));
    assert!(line_bg.affects_line(7));
    assert!(line_bg.affects_line(10));
    assert!(!line_bg.affects_line(11));
}

#[test]
fn test_decoration_hide_type_checks() {
    let hide = Decoration::hide(Span::line(0, 0, 5));
    assert!(hide.is_hide());
    assert!(!hide.is_conceal());
    assert!(!hide.is_line_background());
    assert!(!hide.is_inline_style());
}

#[test]
fn test_decoration_inline_style_type_checks() {
    let inline = Decoration::inline_style(Span::line(0, 0, 5), Style::default());
    assert!(inline.is_inline_style());
    assert!(!inline.is_conceal());
    assert!(!inline.is_line_background());
    assert!(!inline.is_hide());
}
