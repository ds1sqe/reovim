use super::*;

// =============================================================================
// Attributes
// =============================================================================

#[test]
fn attributes_default_is_empty() {
    let a = Attributes::default();
    assert!(a.is_empty());
    assert_eq!(a.bits(), 0);
}

#[test]
fn attributes_bold() {
    let mut a = Attributes::new();
    a.set(Attributes::BOLD);
    assert!(a.contains(Attributes::BOLD));
    assert!(!a.contains(Attributes::ITALIC));
}

#[test]
fn attributes_unset() {
    let mut a = Attributes::BOLD;
    a.set(Attributes::ITALIC);
    a.unset(Attributes::BOLD);
    assert!(!a.contains(Attributes::BOLD));
    assert!(a.contains(Attributes::ITALIC));
}

#[test]
fn attributes_bitor() {
    let a = Attributes::BOLD | Attributes::ITALIC;
    assert!(a.contains(Attributes::BOLD));
    assert!(a.contains(Attributes::ITALIC));
}

#[test]
fn attributes_bitand() {
    let a = Attributes::BOLD | Attributes::ITALIC;
    let b = a & Attributes::BOLD;
    assert!(b.contains(Attributes::BOLD));
    assert!(!b.contains(Attributes::ITALIC));
}

#[test]
fn attributes_from_bits_round_trip() {
    let bits = (Attributes::BOLD | Attributes::UNDERLINE).bits();
    let a = Attributes::from_bits(bits);
    assert!(a.contains(Attributes::BOLD));
    assert!(a.contains(Attributes::UNDERLINE));
    assert!(!a.contains(Attributes::ITALIC));
}

#[test]
fn attributes_all_constants_distinct() {
    let all = [
        Attributes::BOLD,
        Attributes::ITALIC,
        Attributes::UNDERLINE,
        Attributes::STRIKETHROUGH,
        Attributes::REVERSE,
        Attributes::DIM,
    ];
    for (i, a) in all.iter().enumerate() {
        for (j, b) in all.iter().enumerate() {
            if i != j {
                // No two flags share bits.
                assert!((*a & *b).is_empty(), "flags at {i} and {j} share bits");
            }
        }
    }
}

// =============================================================================
// Style
// =============================================================================

#[test]
fn style_default_is_empty() {
    let s = Style::default();
    assert!(s.fg.is_none());
    assert!(s.bg.is_none());
    assert!(s.attributes.is_empty());
}

#[test]
fn style_builder_fg() {
    let c = Color::Red;
    let s = Style::new().fg(c);
    assert_eq!(s.fg, Some(Color::Red));
    assert!(s.bg.is_none());
}

#[test]
fn style_builder_bg() {
    let c = Color::Blue;
    let s = Style::new().bg(c);
    assert!(s.fg.is_none());
    assert_eq!(s.bg, Some(Color::Blue));
}

#[test]
fn style_builder_bold() {
    let s = Style::new().bold();
    assert!(s.attributes.contains(Attributes::BOLD));
}

#[test]
fn style_builder_italic() {
    let s = Style::new().italic();
    assert!(s.attributes.contains(Attributes::ITALIC));
}

#[test]
fn style_builder_underline() {
    let s = Style::new().underline();
    assert!(s.attributes.contains(Attributes::UNDERLINE));
}

#[test]
fn style_builder_dim() {
    let s = Style::new().dim();
    assert!(s.attributes.contains(Attributes::DIM));
}

#[test]
fn style_builder_reverse() {
    let s = Style::new().reverse();
    assert!(s.attributes.contains(Attributes::REVERSE));
}

#[test]
fn style_equality() {
    let a = Style::new().bold();
    let b = Style::new().bold();
    assert_eq!(a, b);
}

// =============================================================================
// Rect
// =============================================================================

#[test]
fn rect_new_fields() {
    let r = Rect::new(1, 2, 10, 20);
    assert_eq!(r.x, 1);
    assert_eq!(r.y, 2);
    assert_eq!(r.width, 10);
    assert_eq!(r.height, 20);
}

#[test]
fn rect_default_is_zero() {
    let r = Rect::default();
    assert_eq!(r.x, 0);
    assert_eq!(r.y, 0);
    assert_eq!(r.width, 0);
    assert_eq!(r.height, 0);
}

#[test]
fn rect_intersect_overlapping() {
    let a = Rect::new(0, 0, 10, 10);
    let b = Rect::new(5, 5, 10, 10);
    let inter = a.intersect(&b).expect("overlapping rects");
    assert_eq!(inter, Rect::new(5, 5, 5, 5));
}

#[test]
fn rect_intersect_non_overlapping() {
    let a = Rect::new(0, 0, 5, 5);
    let b = Rect::new(10, 10, 5, 5);
    assert!(a.intersect(&b).is_none());
}

#[test]
fn rect_contains_point() {
    let r = Rect::new(2, 2, 5, 5);
    assert!(r.contains_point(2, 2));
    assert!(r.contains_point(6, 6));
    assert!(!r.contains_point(7, 7));
    assert!(!r.contains_point(1, 2));
}

#[test]
fn rect_copy() {
    let a = Rect::new(0, 0, 10, 10);
    let b = a;
    assert_eq!(a, b);
}

// =============================================================================
// Insets
// =============================================================================

#[test]
fn insets_zero_constant() {
    assert_eq!(Insets::ZERO.top, 0);
    assert_eq!(Insets::ZERO.bottom, 0);
    assert_eq!(Insets::ZERO.left, 0);
    assert_eq!(Insets::ZERO.right, 0);
}

#[test]
fn insets_new() {
    let i = Insets::new(1, 2, 3, 4);
    assert_eq!(i.top, 1);
    assert_eq!(i.bottom, 2);
    assert_eq!(i.left, 3);
    assert_eq!(i.right, 4);
}

#[test]
fn insets_copy() {
    let a = Insets::new(5, 6, 7, 8);
    let b = a;
    assert_eq!(a, b);
}

// =============================================================================
// ColorDepth + RenderingModel
// =============================================================================

#[test]
fn color_depth_variants_distinct() {
    assert_ne!(ColorDepth::Monochrome, ColorDepth::Ansi16);
    assert_ne!(ColorDepth::Ansi16, ColorDepth::Ansi256);
    assert_ne!(ColorDepth::Ansi256, ColorDepth::TrueColor);
}

#[test]
fn rendering_model_variants_distinct() {
    assert_ne!(RenderingModel::CellGrid, RenderingModel::Canvas);
    assert_ne!(RenderingModel::Canvas, RenderingModel::NativeLayout);
}
