use crate::style::{CellAttrs, CellColor, CellStyle};

#[test]
fn cell_color_default_is_terminal_default() {
    assert_eq!(CellColor::default(), CellColor::Default);
}

#[test]
fn cell_color_rgb_preserves_components() {
    let c = CellColor::Rgb(1, 2, 3);
    assert_eq!(c, CellColor::Rgb(1, 2, 3));
    assert_ne!(c, CellColor::Rgb(3, 2, 1));
}

#[test]
fn cell_color_variants_are_distinct() {
    assert_ne!(CellColor::Default, CellColor::Ansi256(0));
    assert_ne!(CellColor::Named(0), CellColor::Ansi256(0));
    assert_ne!(CellColor::Named(0), CellColor::Rgb(0, 0, 0));
}

#[test]
fn cell_attrs_empty_is_zero() {
    assert_eq!(CellAttrs::empty().bits(), 0);
    assert!(CellAttrs::empty().is_empty());
}

#[test]
fn cell_attrs_individual_flags_have_unique_bits() {
    let all = [
        CellAttrs::BOLD,
        CellAttrs::ITALIC,
        CellAttrs::UNDERLINE,
        CellAttrs::REVERSE,
        CellAttrs::DIM,
    ];
    for (i, a) in all.iter().enumerate() {
        for (j, b) in all.iter().enumerate() {
            if i != j {
                assert!((*a & *b).is_empty(), "{a:?} overlaps {b:?}");
            }
        }
    }
}

#[test]
fn cell_attrs_can_combine_multiple_flags() {
    let combined = CellAttrs::BOLD | CellAttrs::UNDERLINE;
    assert!(combined.contains(CellAttrs::BOLD));
    assert!(combined.contains(CellAttrs::UNDERLINE));
    assert!(!combined.contains(CellAttrs::ITALIC));
}

#[test]
fn cell_attrs_default_is_empty() {
    assert_eq!(CellAttrs::default(), CellAttrs::empty());
}

#[test]
fn cell_style_plain_has_no_colors_no_attrs() {
    let s = CellStyle::plain();
    assert_eq!(s.fg, None);
    assert_eq!(s.bg, None);
    assert!(s.attrs.is_empty());
}

#[test]
fn cell_style_default_equals_plain() {
    assert_eq!(CellStyle::default(), CellStyle::plain());
}

#[test]
fn cell_style_with_fg_sets_foreground_only() {
    let s = CellStyle::plain().with_fg(CellColor::Rgb(255, 0, 0));
    assert_eq!(s.fg, Some(CellColor::Rgb(255, 0, 0)));
    assert_eq!(s.bg, None);
    assert!(s.attrs.is_empty());
}

#[test]
fn cell_style_with_bg_sets_background_only() {
    let s = CellStyle::plain().with_bg(CellColor::Ansi256(42));
    assert_eq!(s.fg, None);
    assert_eq!(s.bg, Some(CellColor::Ansi256(42)));
    assert!(s.attrs.is_empty());
}

#[test]
fn cell_style_with_attrs_replaces_prior_attrs() {
    let s = CellStyle::plain()
        .with_attrs(CellAttrs::BOLD)
        .with_attrs(CellAttrs::ITALIC);
    assert_eq!(s.attrs, CellAttrs::ITALIC);
}

#[test]
fn cell_style_builder_chain_sets_all_fields() {
    let s = CellStyle::plain()
        .with_fg(CellColor::Named(1))
        .with_bg(CellColor::Default)
        .with_attrs(CellAttrs::BOLD | CellAttrs::UNDERLINE);
    assert_eq!(s.fg, Some(CellColor::Named(1)));
    assert_eq!(s.bg, Some(CellColor::Default));
    assert!(s.attrs.contains(CellAttrs::BOLD));
    assert!(s.attrs.contains(CellAttrs::UNDERLINE));
    assert!(!s.attrs.contains(CellAttrs::ITALIC));
}

#[test]
fn cell_style_equality_compares_all_fields() {
    let a = CellStyle::plain().with_fg(CellColor::Named(2));
    let b = CellStyle::plain().with_fg(CellColor::Named(2));
    let c = CellStyle::plain().with_fg(CellColor::Named(3));
    assert_eq!(a, b);
    assert_ne!(a, c);
}

#[test]
fn cell_style_is_copy() {
    fn take_copy(_s: CellStyle) {}
    let s = CellStyle::plain();
    take_copy(s);
    take_copy(s); // would fail to compile if not Copy
}
