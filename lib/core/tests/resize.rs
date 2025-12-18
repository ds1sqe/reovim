//! Resize integration tests
//!
//! Tests for terminal resize behavior and explorer rendering.

use {
    reovim_core::{
        explorer::{ExplorerState, render_explorer},
        highlight::{ColorMode, Theme},
        screen::LayoutManager,
    },
    std::fs::File,
    tempfile::tempdir,
};

/// Strip ANSI escape codes from a string for visible length calculation.
fn strip_ansi_codes(s: &str) -> String {
    let mut result = String::new();
    let mut in_escape = false;

    for c in s.chars() {
        if c == '\x1b' {
            in_escape = true;
        } else if in_escape {
            if c == 'm' {
                in_escape = false;
            }
        } else {
            result.push(c);
        }
    }

    result
}

#[test]
fn test_explorer_lines_padded_at_various_widths() {
    let dir = tempdir().unwrap();
    File::create(dir.path().join("file1.txt")).unwrap();
    File::create(dir.path().join("file2.txt")).unwrap();
    File::create(dir.path().join("longer_filename.rs")).unwrap();

    let state = ExplorerState::new(dir.path().to_path_buf()).unwrap();
    let theme = Theme::default();

    // Test at multiple widths to ensure padding works for all sizes
    for width in [20u16, 30, 40, 50, 80] {
        let lines = render_explorer(&state, width, 15, &theme, ColorMode::Ansi16);

        for (i, line) in lines.iter().enumerate() {
            let visible_len = strip_ansi_codes(line).chars().count();
            assert!(
                visible_len >= width as usize,
                "Width {width}: Line {i} has visible length {visible_len}, expected at least {width}"
            );
        }
    }
}

#[test]
fn test_layout_manager_resize_clamps_explorer() {
    let mut layout = LayoutManager::new(100, 24);
    layout.show_explorer();
    layout.set_explorer_width(30);

    // Resize to smaller screen
    layout.set_screen_size(50, 24);

    // Explorer should be clamped (max 50% of 50 = 25)
    let explorer_layout = layout.explorer_layout().unwrap();
    assert!(
        explorer_layout.width <= 25,
        "Explorer width {} should be clamped to max 25 (50% of 50)",
        explorer_layout.width
    );
}

#[test]
fn test_layout_manager_resize_preserves_visibility() {
    let mut layout = LayoutManager::new(100, 24);
    layout.show_explorer();
    layout.set_explorer_width(25);

    // Resize multiple times
    layout.set_screen_size(80, 20);
    assert!(
        layout.explorer_layout().is_some(),
        "Explorer should remain visible after resize"
    );

    layout.set_screen_size(60, 15);
    assert!(
        layout.explorer_layout().is_some(),
        "Explorer should remain visible after second resize"
    );

    // Even at very small sizes, explorer should be visible (clamped to MIN_WIDTH)
    layout.set_screen_size(30, 10);
    assert!(
        layout.explorer_layout().is_some(),
        "Explorer should remain visible at small size"
    );
}

#[test]
fn test_explorer_render_after_resize_simulation() {
    let dir = tempdir().unwrap();
    File::create(dir.path().join("a.txt")).unwrap();
    File::create(dir.path().join("b.txt")).unwrap();

    let state = ExplorerState::new(dir.path().to_path_buf()).unwrap();
    let theme = Theme::default();

    // Simulate initial render at large size
    let initial_width = 40u16;
    let lines_before = render_explorer(&state, initial_width, 10, &theme, ColorMode::Ansi16);

    // Simulate resize to smaller
    let smaller_width = 25u16;
    let lines_after = render_explorer(&state, smaller_width, 10, &theme, ColorMode::Ansi16);

    // All lines should be properly padded at both sizes
    for line in &lines_before {
        let visible_len = strip_ansi_codes(line).chars().count();
        assert!(visible_len >= initial_width as usize);
    }

    for line in &lines_after {
        let visible_len = strip_ansi_codes(line).chars().count();
        assert!(visible_len >= smaller_width as usize);
    }

    // Lines should be different lengths (shorter after resize)
    let before_len = strip_ansi_codes(&lines_before[0]).len();
    let after_len = strip_ansi_codes(&lines_after[0]).len();
    assert!(
        before_len > after_len,
        "Lines should be shorter after resize: {before_len} vs {after_len}"
    );
}

#[test]
fn test_no_content_bleeding_simulation() {
    // This test simulates the scenario where editor content could "bleed" through
    // explorer gaps if lines aren't properly padded.

    let dir = tempdir().unwrap();
    File::create(dir.path().join("short.txt")).unwrap();

    let state = ExplorerState::new(dir.path().to_path_buf()).unwrap();
    let theme = Theme::default();
    let width = 30u16;
    let height = 20u16;

    let lines = render_explorer(&state, width, height, &theme, ColorMode::Ansi16);

    // With root folder + 1 file = 2 content lines, rest are padding
    // All lines (including padding) must be full width
    assert_eq!(lines.len(), height as usize);

    for (i, line) in lines.iter().enumerate() {
        let stripped = strip_ansi_codes(line);
        let visible_len = stripped.chars().count();

        // Every line must be at least `width` characters
        assert!(
            visible_len >= width as usize,
            "Line {i} has {visible_len} visible chars, expected at least {width} to prevent bleeding. Content: '{stripped}'"
        );
    }
}
