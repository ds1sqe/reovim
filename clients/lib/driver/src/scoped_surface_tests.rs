use std::cell::RefCell;

use crate::{Rect, ChromeSurface, Style, types::Color};

use super::ScopedSurface;

// =============================================================================
// Recording surface for assertions
// =============================================================================

#[derive(Debug, Clone)]
enum Op {
    WriteStyled { x: u16, y: u16, text: String },
    ApplyStyle { x: u16, y: u16 },
    OverlayBg { x: u16, y: u16 },
    Fill { rect: Rect },
    Clear { rect: Rect },
}

struct RecordingSurface {
    ops: RefCell<Vec<Op>>,
    width: u16,
    height: u16,
}

impl RecordingSurface {
    fn new(width: u16, height: u16) -> Self {
        Self {
            ops: RefCell::new(Vec::new()),
            width,
            height,
        }
    }

    fn ops(&self) -> Vec<Op> {
        self.ops.borrow().clone()
    }
}

impl ChromeSurface for RecordingSurface {
    #[allow(clippy::cast_possible_truncation)]
    fn write_styled(&mut self, x: u16, y: u16, text: &str, _style: Style) -> u16 {
        self.ops.borrow_mut().push(Op::WriteStyled {
            x,
            y,
            text: text.to_string(),
        });
        text.len() as u16
    }

    fn apply_style(&mut self, x: u16, y: u16, _style: Style) {
        self.ops.borrow_mut().push(Op::ApplyStyle { x, y });
    }

    fn overlay_bg(&mut self, x: u16, y: u16, _bg: Color) {
        self.ops.borrow_mut().push(Op::OverlayBg { x, y });
    }

    fn fill(&mut self, rect: Rect, _ch: char, _style: Style) {
        self.ops.borrow_mut().push(Op::Fill { rect });
    }

    fn clear(&mut self, rect: Rect) {
        self.ops.borrow_mut().push(Op::Clear { rect });
    }

    fn size(&self) -> (u16, u16) {
        (self.width, self.height)
    }
}

// =============================================================================
// ScopedSurface coordinate offset
// =============================================================================

#[test]
fn write_offset_to_bounds_origin() {
    let mut surface = RecordingSurface::new(80, 24);
    let bounds = Rect::new(10, 5, 20, 10);
    {
        let mut scoped = ScopedSurface::new(&mut surface, bounds);
        scoped.write_styled(0, 0, "hi", Style::default());
    }
    let ops = surface.ops();
    assert_eq!(ops.len(), 1);
    match &ops[0] {
        Op::WriteStyled { x, y, text } => {
            assert_eq!(*x, 10);
            assert_eq!(*y, 5);
            assert_eq!(text, "hi");
        }
        _ => panic!("expected WriteStyled"),
    }
}

#[test]
fn write_at_nonzero_local_coords() {
    let mut surface = RecordingSurface::new(80, 24);
    let bounds = Rect::new(10, 5, 20, 10);
    {
        let mut scoped = ScopedSurface::new(&mut surface, bounds);
        scoped.write_styled(3, 2, "abc", Style::default());
    }
    match &surface.ops()[0] {
        Op::WriteStyled { x, y, text } => {
            assert_eq!(*x, 13);
            assert_eq!(*y, 7);
            assert_eq!(text, "abc");
        }
        _ => panic!("expected WriteStyled"),
    }
}

// =============================================================================
// ScopedSurface clipping
// =============================================================================

#[test]
fn write_clipped_when_x_outside_bounds() {
    let mut surface = RecordingSurface::new(80, 24);
    let bounds = Rect::new(10, 5, 5, 5);
    {
        let mut scoped = ScopedSurface::new(&mut surface, bounds);
        let cols = scoped.write_styled(5, 0, "clipped", Style::default());
        assert_eq!(cols, 0);
    }
    assert!(surface.ops().is_empty());
}

#[test]
fn write_clipped_when_y_outside_bounds() {
    let mut surface = RecordingSurface::new(80, 24);
    let bounds = Rect::new(0, 0, 10, 3);
    {
        let mut scoped = ScopedSurface::new(&mut surface, bounds);
        let cols = scoped.write_styled(0, 3, "clipped", Style::default());
        assert_eq!(cols, 0);
    }
    assert!(surface.ops().is_empty());
}

#[test]
fn write_truncated_to_fit_width() {
    let mut surface = RecordingSurface::new(80, 24);
    let bounds = Rect::new(0, 0, 5, 1);
    {
        let mut scoped = ScopedSurface::new(&mut surface, bounds);
        scoped.write_styled(0, 0, "hello world", Style::default());
    }
    match &surface.ops()[0] {
        Op::WriteStyled { text, .. } => {
            assert_eq!(text, "hello");
        }
        _ => panic!("expected WriteStyled"),
    }
}

#[test]
fn apply_style_offset_and_clip() {
    let mut surface = RecordingSurface::new(80, 24);
    let bounds = Rect::new(5, 5, 10, 10);
    {
        let mut scoped = ScopedSurface::new(&mut surface, bounds);
        scoped.apply_style(2, 3, Style::default());
        scoped.apply_style(10, 0, Style::default()); // clipped
    }
    let ops = surface.ops();
    assert_eq!(ops.len(), 1);
    match &ops[0] {
        Op::ApplyStyle { x, y } => {
            assert_eq!(*x, 7);
            assert_eq!(*y, 8);
        }
        _ => panic!("expected ApplyStyle"),
    }
}

#[test]
fn overlay_bg_offset_and_clip() {
    let mut surface = RecordingSurface::new(80, 24);
    let bounds = Rect::new(10, 10, 5, 5);
    {
        let mut scoped = ScopedSurface::new(&mut surface, bounds);
        scoped.overlay_bg(1, 1, Color::Red);
        scoped.overlay_bg(5, 0, Color::Blue); // clipped
    }
    let ops = surface.ops();
    assert_eq!(ops.len(), 1);
    match &ops[0] {
        Op::OverlayBg { x, y } => {
            assert_eq!(*x, 11);
            assert_eq!(*y, 11);
        }
        _ => panic!("expected OverlayBg"),
    }
}

// =============================================================================
// ScopedSurface fill and clear with intersection
// =============================================================================

#[test]
fn fill_intersects_with_bounds() {
    let mut surface = RecordingSurface::new(80, 24);
    let bounds = Rect::new(10, 10, 10, 10);
    {
        let mut scoped = ScopedSurface::new(&mut surface, bounds);
        // Local rect (0,0,5,5) -> absolute (10,10,5,5) which is within bounds
        scoped.fill(Rect::new(0, 0, 5, 5), ' ', Style::default());
    }
    let ops = surface.ops();
    assert_eq!(ops.len(), 1);
    match &ops[0] {
        Op::Fill { rect } => {
            assert_eq!(*rect, Rect::new(10, 10, 5, 5));
        }
        _ => panic!("expected Fill"),
    }
}

#[test]
fn fill_no_overlap_is_noop() {
    let mut surface = RecordingSurface::new(80, 24);
    let bounds = Rect::new(10, 10, 5, 5);
    {
        let mut scoped = ScopedSurface::new(&mut surface, bounds);
        // Local rect at (20,20) -> absolute (30,30) which is outside bounds
        scoped.fill(Rect::new(20, 20, 5, 5), ' ', Style::default());
    }
    assert!(surface.ops().is_empty());
}

#[test]
fn clear_intersects_with_bounds() {
    let mut surface = RecordingSurface::new(80, 24);
    let bounds = Rect::new(5, 5, 10, 10);
    {
        let mut scoped = ScopedSurface::new(&mut surface, bounds);
        scoped.clear(Rect::new(0, 0, 20, 20));
    }
    let ops = surface.ops();
    assert_eq!(ops.len(), 1);
    match &ops[0] {
        Op::Clear { rect } => {
            // Intersection of (5,5,10,10) and (5,5,20,20) = (5,5,10,10)
            assert_eq!(*rect, Rect::new(5, 5, 10, 10));
        }
        _ => panic!("expected Clear"),
    }
}

// =============================================================================
// ScopedSurface size
// =============================================================================

#[test]
fn size_returns_bounds_dimensions() {
    let mut surface = RecordingSurface::new(80, 24);
    let bounds = Rect::new(10, 5, 30, 12);
    let scoped = ScopedSurface::new(&mut surface, bounds);
    assert_eq!(scoped.size(), (30, 12));
}

// =============================================================================
// Rect helpers
// =============================================================================

#[test]
fn rect_intersect_full_overlap() {
    let a = Rect::new(0, 0, 10, 10);
    let b = Rect::new(0, 0, 10, 10);
    assert_eq!(a.intersect(&b), Some(Rect::new(0, 0, 10, 10)));
}

#[test]
fn rect_intersect_partial_overlap() {
    let a = Rect::new(0, 0, 10, 10);
    let b = Rect::new(5, 5, 10, 10);
    assert_eq!(a.intersect(&b), Some(Rect::new(5, 5, 5, 5)));
}

#[test]
fn rect_intersect_no_overlap() {
    let a = Rect::new(0, 0, 5, 5);
    let b = Rect::new(10, 10, 5, 5);
    assert_eq!(a.intersect(&b), None);
}

#[test]
fn rect_intersect_adjacent_no_overlap() {
    let a = Rect::new(0, 0, 5, 5);
    let b = Rect::new(5, 0, 5, 5);
    assert_eq!(a.intersect(&b), None);
}

#[test]
fn rect_intersect_one_inside_other() {
    let outer = Rect::new(0, 0, 20, 20);
    let inner = Rect::new(5, 5, 3, 3);
    assert_eq!(outer.intersect(&inner), Some(Rect::new(5, 5, 3, 3)));
}

#[test]
fn rect_contains_point_inside() {
    let r = Rect::new(10, 10, 5, 5);
    assert!(r.contains_point(10, 10));
    assert!(r.contains_point(14, 14));
    assert!(r.contains_point(12, 12));
}

#[test]
fn rect_contains_point_outside() {
    let r = Rect::new(10, 10, 5, 5);
    assert!(!r.contains_point(9, 10));
    assert!(!r.contains_point(10, 9));
    assert!(!r.contains_point(15, 10));
    assert!(!r.contains_point(10, 15));
}

#[test]
fn rect_contains_point_zero_size() {
    let r = Rect::new(5, 5, 0, 0);
    assert!(!r.contains_point(5, 5));
}

// =============================================================================
// MC/DC branch: empty text after truncation
// =============================================================================

#[test]
fn write_styled_empty_text_is_noop() {
    let mut surface = RecordingSurface::new(80, 24);
    let bounds = Rect::new(0, 0, 10, 10);
    {
        let mut scoped = ScopedSurface::new(&mut surface, bounds);
        let cols = scoped.write_styled(0, 0, "", Style::default());
        assert_eq!(cols, 0);
    }
    assert!(surface.ops().is_empty());
}

// =============================================================================
// MC/DC branch: y-only clipping
// =============================================================================

#[test]
fn apply_style_clipped_by_y_alone() {
    let mut surface = RecordingSurface::new(80, 24);
    let bounds = Rect::new(0, 0, 10, 5);
    {
        let mut scoped = ScopedSurface::new(&mut surface, bounds);
        // x=0 is within bounds, but y=5 is outside height=5
        scoped.apply_style(0, 5, Style::default());
    }
    assert!(surface.ops().is_empty());
}

#[test]
fn overlay_bg_clipped_by_y_alone() {
    let mut surface = RecordingSurface::new(80, 24);
    let bounds = Rect::new(0, 0, 10, 5);
    {
        let mut scoped = ScopedSurface::new(&mut surface, bounds);
        scoped.overlay_bg(0, 5, Color::Red);
    }
    assert!(surface.ops().is_empty());
}

// =============================================================================
// MC/DC branch: clear with no intersection
// =============================================================================

#[test]
fn clear_no_overlap_is_noop() {
    let mut surface = RecordingSurface::new(80, 24);
    let bounds = Rect::new(10, 10, 5, 5);
    {
        let mut scoped = ScopedSurface::new(&mut surface, bounds);
        scoped.clear(Rect::new(50, 50, 5, 5));
    }
    assert!(surface.ops().is_empty());
}
