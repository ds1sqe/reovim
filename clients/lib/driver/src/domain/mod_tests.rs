use {super::*, crate::testing::surface::TestChromeGrid};

#[test]
fn unknown_domain_placeholder_renders_label() {
    let mut surface = TestChromeGrid::new(80, 24);
    let bounds = crate::Rect::new(0, 0, 40, 5);
    render_unknown_domain_placeholder(&mut surface, bounds, DomainId(99));
    assert!(surface.has_content());
}

#[test]
fn unknown_domain_placeholder_empty_bounds_skips() {
    let mut surface = TestChromeGrid::new(80, 24);
    let bounds = crate::Rect::new(0, 0, 0, 0);
    render_unknown_domain_placeholder(&mut surface, bounds, DomainId(99));
    assert!(!surface.has_content());
}
