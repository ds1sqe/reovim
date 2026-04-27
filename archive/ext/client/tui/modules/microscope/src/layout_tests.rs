use super::*;

#[test]
fn standard_layout() {
    let bounds = LayoutBounds::calculate(80, 24);
    assert!(bounds.total_height >= MIN_HEIGHT);
    assert!(bounds.show_preview);
    assert!(bounds.results_width > 0);
    assert!(bounds.preview_width > 0);
    assert_eq!(bounds.x, 0);
    assert_eq!(bounds.width, 80);
}

#[test]
fn wide_terminal_shows_preview() {
    let bounds = LayoutBounds::calculate(100, 40);
    assert!(bounds.show_preview);
    assert!(bounds.preview_width > 0);
    assert_eq!(bounds.width, 100);
}

#[test]
fn narrow_terminal_hides_preview() {
    let bounds = LayoutBounds::calculate(50, 24);
    assert!(!bounds.show_preview);
    assert_eq!(bounds.preview_width, 0);
    assert_eq!(bounds.results_width, 50);
}

#[test]
fn very_small_terminal() {
    let bounds = LayoutBounds::calculate(20, 5);
    assert!(bounds.total_height <= 5);
}

#[test]
fn minimum_height_constraint() {
    // Even with small terminal height, total_height >= MIN_HEIGHT (clamped to terminal).
    let bounds = LayoutBounds::calculate(80, 4);
    // MIN_HEIGHT is 6, but terminal is only 4, so clamped to 4.
    assert_eq!(bounds.total_height, 4);

    let bounds = LayoutBounds::calculate(80, 10);
    assert!(bounds.total_height >= MIN_HEIGHT);
}

#[test]
fn query_row_position() {
    let bounds = LayoutBounds::calculate(80, 24);
    assert_eq!(bounds.query_row, bounds.y);
    assert_eq!(bounds.panel_start_y, bounds.y + 2);
}

#[test]
fn panel_height() {
    let bounds = LayoutBounds::calculate(80, 30);
    assert_eq!(bounds.panel_height, bounds.total_height.saturating_sub(2));
}

#[test]
fn preview_x_after_separator() {
    let bounds = LayoutBounds::calculate(100, 40);
    if bounds.show_preview {
        assert_eq!(bounds.preview_x, bounds.results_width + 1);
    }
}

#[test]
fn exact_min_preview_width() {
    let bounds = LayoutBounds::calculate(MIN_PREVIEW_WIDTH, 24);
    assert!(bounds.show_preview);

    let bounds_below = LayoutBounds::calculate(MIN_PREVIEW_WIDTH - 1, 24);
    assert!(!bounds_below.show_preview);
}

#[test]
fn layout_debug() {
    let bounds = LayoutBounds::calculate(80, 24);
    let debug = format!("{bounds:?}");
    assert!(debug.contains("LayoutBounds"));
}

#[test]
fn layout_clone() {
    let bounds = LayoutBounds::calculate(80, 24);
    #[allow(clippy::redundant_clone)]
    let cloned = bounds.clone();
    assert_eq!(cloned.width, bounds.width);
}
