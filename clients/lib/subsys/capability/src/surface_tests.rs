use {
    super::*,
    crate::draw::{Color, Rect, Style},
};

struct MockSurface {
    width: u16,
    height: u16,
    writes: Vec<(u16, u16, String)>,
    fills: Vec<Rect>,
    clears: Vec<Rect>,
}

impl MockSurface {
    fn new(width: u16, height: u16) -> Self {
        Self {
            width,
            height,
            writes: Vec::new(),
            fills: Vec::new(),
            clears: Vec::new(),
        }
    }
}

impl ChromeSurface for MockSurface {
    fn write_styled(&mut self, x: u16, y: u16, text: &str, _style: Style) -> u16 {
        self.writes.push((x, y, text.to_string()));
        #[allow(clippy::cast_possible_truncation)]
        {
            text.len() as u16
        }
    }

    fn apply_style(&mut self, _x: u16, _y: u16, _style: Style) {}

    fn overlay_bg(&mut self, _x: u16, _y: u16, _bg: Color) {}

    fn fill(&mut self, rect: Rect, _ch: char, _style: Style) {
        self.fills.push(rect);
    }

    fn clear(&mut self, rect: Rect) {
        self.clears.push(rect);
    }

    fn size(&self) -> (u16, u16) {
        (self.width, self.height)
    }
}

#[test]
fn chrome_surface_write_styled() {
    let mut s = MockSurface::new(80, 24);
    let cols = s.write_styled(0, 0, "hello", Style::default());
    assert_eq!(cols, 5);
    assert_eq!(s.writes.len(), 1);
    assert_eq!(s.writes[0], (0, 0, "hello".to_string()));
}

#[test]
fn chrome_surface_size() {
    let s = MockSurface::new(120, 40);
    assert_eq!(s.size(), (120, 40));
}

#[test]
fn chrome_surface_fill() {
    let mut s = MockSurface::new(80, 24);
    let r = Rect::new(0, 0, 10, 5);
    s.fill(r, ' ', Style::default());
    assert_eq!(s.fills.len(), 1);
    assert_eq!(s.fills[0], r);
}

#[test]
fn chrome_surface_clear() {
    let mut s = MockSurface::new(80, 24);
    let r = Rect::new(5, 5, 20, 10);
    s.clear(r);
    assert_eq!(s.clears.len(), 1);
    assert_eq!(s.clears[0], r);
}

#[test]
fn chrome_surface_object_safety() {
    let mut s: Box<dyn ChromeSurface> = Box::new(MockSurface::new(80, 24));
    let _ = s.size();
    let _ = s.write_styled(0, 0, "x", Style::default());
}

#[test]
fn chrome_surface_apply_style_no_panic() {
    let mut s = MockSurface::new(80, 24);
    s.apply_style(0, 0, Style::new().bold());
}

#[test]
fn chrome_surface_overlay_bg_no_panic() {
    let mut s = MockSurface::new(80, 24);
    s.overlay_bg(0, 0, Color::Red);
}
