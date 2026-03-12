use super::*;

#[test]
fn test_border_style_chars() {
    assert!(BorderStyle::None.chars().is_none());
    assert!(BorderStyle::Single.chars().is_some());
    assert!(BorderStyle::Double.chars().is_some());
    assert!(BorderStyle::Rounded.chars().is_some());
    assert!(BorderStyle::Bold.chars().is_some());
}

#[test]
fn test_border_chars_single() {
    let chars = BorderChars::SINGLE;
    assert_eq!(chars.top_left, '┌');
    assert_eq!(chars.horizontal, '─');
    assert_eq!(chars.vertical, '│');
    assert_eq!(chars.cross, '┼');
}

#[test]
fn test_border_chars_double() {
    let chars = BorderChars::DOUBLE;
    assert_eq!(chars.top_left, '╔');
    assert_eq!(chars.horizontal, '═');
    assert_eq!(chars.vertical, '║');
}

#[test]
fn test_border_chars_rounded() {
    let chars = BorderChars::ROUNDED;
    assert_eq!(chars.top_left, '╭');
    assert_eq!(chars.bottom_right, '╯');
}

#[test]
fn test_border_chars_bold() {
    let chars = BorderChars::BOLD;
    assert_eq!(chars.top_left, '┏');
    assert_eq!(chars.horizontal, '━');
    assert_eq!(chars.vertical, '┃');
}

#[test]
fn test_window_adjacency_none() {
    let adj = WindowAdjacency::none();
    assert!(!adj.left);
    assert!(!adj.right);
    assert!(!adj.top);
    assert!(!adj.bottom);
}

#[test]
fn test_window_adjacency_compute_left() {
    let target = Rect::new(10, 0, 10, 10);
    let windows = vec![
        Rect::new(0, 0, 10, 10), // Left of target
        target,
    ];
    let adj = WindowAdjacency::compute(&target, &windows);
    assert!(adj.left);
    assert!(!adj.right);
}

#[test]
fn test_window_adjacency_compute_right() {
    let target = Rect::new(0, 0, 10, 10);
    let windows = vec![
        target,
        Rect::new(10, 0, 10, 10), // Right of target
    ];
    let adj = WindowAdjacency::compute(&target, &windows);
    assert!(!adj.left);
    assert!(adj.right);
}

#[test]
fn test_window_adjacency_compute_vertical() {
    let target = Rect::new(0, 10, 10, 10);
    let windows = vec![
        Rect::new(0, 0, 10, 10), // Above
        target,
        Rect::new(0, 20, 10, 10), // Below
    ];
    let adj = WindowAdjacency::compute(&target, &windows);
    assert!(adj.top);
    assert!(adj.bottom);
}

#[test]
fn test_select_corner_char_no_adjacency() {
    let chars = BorderChars::SINGLE;
    let adj = WindowAdjacency::none();

    assert_eq!(select_corner_char(&chars, Corner::TopLeft, adj), '┌');
    assert_eq!(select_corner_char(&chars, Corner::TopRight, adj), '┐');
    assert_eq!(select_corner_char(&chars, Corner::BottomLeft, adj), '└');
    assert_eq!(select_corner_char(&chars, Corner::BottomRight, adj), '┘');
}

#[test]
fn test_select_corner_char_with_adjacency() {
    let chars = BorderChars::SINGLE;

    // Top-left with left adjacency -> tee_left
    let adj = WindowAdjacency {
        left: true,
        ..WindowAdjacency::none()
    };
    assert_eq!(select_corner_char(&chars, Corner::TopLeft, adj), '├');

    // Top-left with top adjacency -> tee_top
    let adj = WindowAdjacency {
        top: true,
        ..WindowAdjacency::none()
    };
    assert_eq!(select_corner_char(&chars, Corner::TopLeft, adj), '┬');

    // Top-left with both -> cross
    let adj = WindowAdjacency {
        left: true,
        top: true,
        ..WindowAdjacency::none()
    };
    assert_eq!(select_corner_char(&chars, Corner::TopLeft, adj), '┼');
}

#[test]
fn test_render_border_simple() {
    let mut buffer = FrameBuffer::new(10, 10);
    render_border_simple(
        &mut buffer,
        Rect::new(0, 0, 10, 10),
        BorderStyle::Single,
        &Style::default(),
    );

    // Check corners
    assert_eq!(buffer.get(0, 0).unwrap().char, '┌');
    assert_eq!(buffer.get(9, 0).unwrap().char, '┐');
    assert_eq!(buffer.get(0, 9).unwrap().char, '└');
    assert_eq!(buffer.get(9, 9).unwrap().char, '┘');

    // Check edges
    assert_eq!(buffer.get(5, 0).unwrap().char, '─');
    assert_eq!(buffer.get(0, 5).unwrap().char, '│');
}

#[test]
fn test_render_border_with_adjacency() {
    let mut buffer = FrameBuffer::new(20, 10);
    let adj = WindowAdjacency {
        right: true,
        ..WindowAdjacency::none()
    };
    render_border(
        &mut buffer,
        Rect::new(0, 0, 10, 10),
        BorderStyle::Single,
        &adj,
        &Style::default(),
    );

    // Top-right with right adjacency -> tee_right
    assert_eq!(buffer.get(9, 0).unwrap().char, '┤');
    // Bottom-right with right adjacency -> tee_right
    assert_eq!(buffer.get(9, 9).unwrap().char, '┤');
}

#[test]
fn test_render_border_none() {
    let mut buffer = FrameBuffer::new(10, 10);
    buffer.set(0, 0, Cell::from_char('X'));
    render_border_simple(
        &mut buffer,
        Rect::new(0, 0, 10, 10),
        BorderStyle::None,
        &Style::default(),
    );

    // Should not change anything
    assert_eq!(buffer.get(0, 0).unwrap().char, 'X');
}

#[test]
fn test_render_border_too_small() {
    let mut buffer = FrameBuffer::new(10, 10);
    buffer.set(0, 0, Cell::from_char('X'));

    // 1x1 is too small for a border
    render_border_simple(
        &mut buffer,
        Rect::new(0, 0, 1, 1),
        BorderStyle::Single,
        &Style::default(),
    );
    assert_eq!(buffer.get(0, 0).unwrap().char, 'X'); // Unchanged
}

#[test]
fn test_inner_bounds() {
    let bounds = Rect::new(0, 0, 10, 10);

    // No border -> same bounds
    let inner = inner_bounds(bounds, BorderStyle::None);
    assert_eq!(inner, bounds);

    // With border -> shrink by 1 on all sides
    let inner = inner_bounds(bounds, BorderStyle::Single);
    assert_eq!(inner.x, 1);
    assert_eq!(inner.y, 1);
    assert_eq!(inner.width, 8);
    assert_eq!(inner.height, 8);
}

#[test]
fn test_inner_bounds_small() {
    let bounds = Rect::new(0, 0, 2, 2);
    let inner = inner_bounds(bounds, BorderStyle::Single);
    assert_eq!(inner.width, 0);
    assert_eq!(inner.height, 0);
}

#[test]
fn test_ranges_overlap() {
    // Overlapping
    assert!(ranges_overlap(0, 10, 5, 15));
    assert!(ranges_overlap(5, 15, 0, 10));

    // Touching (not overlapping)
    assert!(!ranges_overlap(0, 10, 10, 20));

    // Not touching
    assert!(!ranges_overlap(0, 10, 20, 30));
}

#[test]
fn test_border_style_default() {
    let style = BorderStyle::default();
    assert_eq!(style, BorderStyle::None);
}

#[test]
fn test_border_mode_default() {
    let mode = BorderMode::default();
    assert_eq!(mode, BorderMode::Always);
}

// =========================================================================
// MC/DC coverage for WindowAdjacency::compute and render_border
// =========================================================================

#[test]
fn test_adjacency_edge_touches_but_no_vertical_overlap() {
    // Left edge touches (other.x + other.width == target.x) but
    // ranges don't overlap vertically -> left stays false.
    let target = Rect::new(10, 0, 10, 5);
    let windows = vec![
        Rect::new(0, 10, 10, 5), // Left edge touches at x=10, but y ranges don't overlap
        target,
    ];
    let adj = WindowAdjacency::compute(&target, &windows);
    assert!(!adj.left);
}

#[test]
fn test_adjacency_right_edge_touches_no_overlap() {
    // Right edge touches but no vertical overlap.
    let target = Rect::new(0, 0, 10, 5);
    let windows = vec![
        target,
        Rect::new(10, 10, 10, 5), // Right edge at x=10, y doesn't overlap
    ];
    let adj = WindowAdjacency::compute(&target, &windows);
    assert!(!adj.right);
}

#[test]
fn test_adjacency_top_edge_touches_no_overlap() {
    // Top edge touches but no horizontal overlap.
    let target = Rect::new(0, 10, 5, 10);
    let windows = vec![
        Rect::new(10, 0, 5, 10), // Bottom at y=10 matches target.y, but x doesn't overlap
        target,
    ];
    let adj = WindowAdjacency::compute(&target, &windows);
    assert!(!adj.top);
}

#[test]
fn test_adjacency_bottom_edge_touches_no_overlap() {
    // Bottom edge touches but no horizontal overlap.
    let target = Rect::new(0, 0, 5, 10);
    let windows = vec![
        target,
        Rect::new(10, 10, 5, 10), // Top at y=10 matches target bottom, but x doesn't overlap
    ];
    let adj = WindowAdjacency::compute(&target, &windows);
    assert!(!adj.bottom);
}

#[test]
fn test_ranges_overlap_first_true_second_false() {
    // a_start < b_end is true, but b_start >= a_end is true (not overlapping)
    // ranges_overlap(0, 5, 5, 10): a_start(0) < b_end(10) = true, b_start(5) < a_end(5) = false
    assert!(!ranges_overlap(0, 5, 5, 10));
}

#[test]
fn test_render_border_width_ok_height_too_small() {
    let mut buffer = FrameBuffer::new(10, 10);
    buffer.set(0, 0, Cell::from_char('X'));
    // width >= 2 but height < 2
    render_border_simple(
        &mut buffer,
        Rect::new(0, 0, 5, 1),
        BorderStyle::Single,
        &Style::default(),
    );
    assert_eq!(buffer.get(0, 0).unwrap().char, 'X'); // Unchanged
}

// =========================================================================
// Coverage tests for uncovered select_corner_char branches
// =========================================================================

/// Test `TopRight` corner with top adjacency -> `tee_top` (line 288).
#[test]
fn test_select_corner_top_right_with_top() {
    let chars = BorderChars::SINGLE;
    let adj = WindowAdjacency {
        top: true,
        ..WindowAdjacency::none()
    };
    assert_eq!(select_corner_char(&chars, Corner::TopRight, adj), '┬');
}

/// Test `TopRight` corner with both right and top adjacency -> cross (line 289).
#[test]
fn test_select_corner_top_right_with_both() {
    let chars = BorderChars::SINGLE;
    let adj = WindowAdjacency {
        right: true,
        top: true,
        ..WindowAdjacency::none()
    };
    assert_eq!(select_corner_char(&chars, Corner::TopRight, adj), '┼');
}

/// Test `BottomLeft` corner with left adjacency -> `tee_left` (line 293).
#[test]
fn test_select_corner_bottom_left_with_left() {
    let chars = BorderChars::SINGLE;
    let adj = WindowAdjacency {
        left: true,
        ..WindowAdjacency::none()
    };
    assert_eq!(select_corner_char(&chars, Corner::BottomLeft, adj), '├');
}

/// Test `BottomLeft` corner with bottom adjacency -> `tee_bottom` (line 294).
#[test]
fn test_select_corner_bottom_left_with_bottom() {
    let chars = BorderChars::SINGLE;
    let adj = WindowAdjacency {
        bottom: true,
        ..WindowAdjacency::none()
    };
    assert_eq!(select_corner_char(&chars, Corner::BottomLeft, adj), '┴');
}

/// Test `BottomLeft` corner with both left and bottom adjacency -> cross (line 295).
#[test]
fn test_select_corner_bottom_left_with_both() {
    let chars = BorderChars::SINGLE;
    let adj = WindowAdjacency {
        left: true,
        bottom: true,
        ..WindowAdjacency::none()
    };
    assert_eq!(select_corner_char(&chars, Corner::BottomLeft, adj), '┼');
}

/// Test `BottomRight` corner with bottom adjacency -> `tee_bottom` (line 300).
#[test]
fn test_select_corner_bottom_right_with_bottom() {
    let chars = BorderChars::SINGLE;
    let adj = WindowAdjacency {
        bottom: true,
        ..WindowAdjacency::none()
    };
    assert_eq!(select_corner_char(&chars, Corner::BottomRight, adj), '┴');
}

/// Test `BottomRight` corner with both right and bottom adjacency -> cross (line 301).
#[test]
fn test_select_corner_bottom_right_with_both() {
    let chars = BorderChars::SINGLE;
    let adj = WindowAdjacency {
        right: true,
        bottom: true,
        ..WindowAdjacency::none()
    };
    assert_eq!(select_corner_char(&chars, Corner::BottomRight, adj), '┼');
}

// ================================================================
// WindowAdjacency constants and BitOr
// ================================================================

#[test]
fn test_adjacency_directional_constants() {
    const {
        assert!(WindowAdjacency::LEFT.left);
        assert!(!WindowAdjacency::LEFT.right);

        assert!(WindowAdjacency::RIGHT.right);
        assert!(!WindowAdjacency::RIGHT.left);

        assert!(WindowAdjacency::TOP.top);
        assert!(!WindowAdjacency::TOP.bottom);

        assert!(WindowAdjacency::BOTTOM.bottom);
        assert!(!WindowAdjacency::BOTTOM.top);
    }
}

#[test]
fn test_adjacency_none_and_all() {
    const {
        assert!(
            !WindowAdjacency::NONE.left
                && !WindowAdjacency::NONE.right
                && !WindowAdjacency::NONE.top
                && !WindowAdjacency::NONE.bottom
        );
        assert!(
            WindowAdjacency::ALL.left
                && WindowAdjacency::ALL.right
                && WindowAdjacency::ALL.top
                && WindowAdjacency::ALL.bottom
        );
    }
}

#[test]
fn test_adjacency_bitor_combines_directions() {
    let adj = WindowAdjacency::LEFT | WindowAdjacency::TOP;
    assert!(adj.left);
    assert!(adj.top);
    assert!(!adj.right);
    assert!(!adj.bottom);
}

#[test]
fn test_adjacency_bitor_all_four() {
    let adj = WindowAdjacency::LEFT
        | WindowAdjacency::RIGHT
        | WindowAdjacency::TOP
        | WindowAdjacency::BOTTOM;
    assert!(adj.left && adj.right && adj.top && adj.bottom);
}
