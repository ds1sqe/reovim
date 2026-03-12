use {
    super::*,
    crate::{Color, statusline::SectionId},
};

#[test]
fn test_render_statusline_simple() {
    let mut buffer = FrameBuffer::new(80, 24);
    let mode_style = Style::new().bg(Color::Blue).fg(Color::Black);
    let pos_style = Style::new().bg(Color::Blue).fg(Color::Black);
    let fill_style = Style::new().bg(Color::Grey);

    render_statusline_simple(
        &mut buffer,
        23,
        " NORMAL ",
        " 42:15 ",
        &mode_style,
        &pos_style,
        &fill_style,
    );

    // Check mode section
    assert_eq!(buffer.get(1, 23).unwrap().char, 'N');
    assert_eq!(buffer.get(2, 23).unwrap().char, 'O');

    // Check position section (at right edge)
    assert_eq!(buffer.get(79, 23).unwrap().char, ' ');
    assert_eq!(buffer.get(78, 23).unwrap().char, '5');
}

#[test]
fn test_render_sections() {
    let mut buffer = FrameBuffer::new(80, 24);
    let style_a = Style::new().bg(Color::Blue);
    let style_z = Style::new().bg(Color::Green);

    let sections = vec![
        Section::new(SectionId::A, " NORMAL ", style_a),
        Section::new(SectionId::Z, " 42:15 ", style_z),
    ];

    let config = StatuslineRendererConfig {
        separator: StatuslineSeparator::NONE,
        ..Default::default()
    };

    render_sections(&mut buffer, 23, &sections, &config);

    // Check section A on left
    assert_eq!(buffer.get(1, 23).unwrap().char, 'N');

    // Check section Z on right
    let z_start = 80 - 7; // " 42:15 " is 7 chars
    assert_eq!(buffer.get(z_start + 1, 23).unwrap().char, '4');
}

#[test]
fn test_separator_constants() {
    assert_eq!(StatuslineSeparator::POWERLINE_ARROW.left, "\u{e0b0}");
    assert_eq!(StatuslineSeparator::PIPE.left, "|");
    assert!(StatuslineSeparator::NONE.left.is_empty());
}

#[test]
fn test_truncate_sections_no_truncation_needed() {
    let sections = vec![
        Section::new(SectionId::A, "ABC", Style::default()),
        Section::new(SectionId::Z, "XYZ", Style::default()),
    ];

    let result = truncate_sections(&sections, 10);
    assert_eq!(result.len(), 2);
    assert_eq!(result[0].text, "ABC");
    assert_eq!(result[1].text, "XYZ");
}

#[test]
fn test_truncate_sections_truncates_lowest_priority() {
    let sections = vec![
        Section::with_priority(SectionId::A, "NORMAL", Style::default(), 200), // High priority
        Section::with_priority(SectionId::C, "very-long-filename.rs", Style::default(), 50), // Low priority
        Section::with_priority(SectionId::Z, "42:15", Style::default(), 150), // Medium priority
    ];

    // Total width: 6 + 21 + 5 = 32
    // Available: 20
    // Need to remove: 12
    let result = truncate_sections(&sections, 20);

    // High priority section (A) should be unchanged
    assert_eq!(result[0].text, "NORMAL");

    // Low priority section (C) should be truncated
    assert!(result[1].text.len() < 21);
    assert!(result[1].text.ends_with('…'));

    // Medium priority section (Z) should be unchanged if enough was removed from C
}

#[test]
fn test_truncate_sections_exact_fit() {
    let sections = vec![Section::new(SectionId::A, "ABCDE", Style::default())];

    let result = truncate_sections(&sections, 5);
    assert_eq!(result[0].text, "ABCDE");
}

#[test]
fn test_truncate_text_to_width() {
    assert_eq!(truncate_text_to_width("hello world", 5), "hello");
    assert_eq!(truncate_text_to_width("hello", 10), "hello");
    assert_eq!(truncate_text_to_width("hello", 0), "");
}

// =========================================================================
// Extended statusline renderer tests
// =========================================================================

#[test]
fn test_render_sections_with_separators() {
    let mut buffer = FrameBuffer::new(80, 24);
    let style_a = Style::new().bg(Color::Blue);

    let sections = vec![
        Section::new(SectionId::A, " NORMAL ", style_a.clone()),
        Section::new(SectionId::B, " main.rs ", style_a),
    ];

    let config = StatuslineRendererConfig {
        separator: StatuslineSeparator::PIPE,
        powerline_coloring: false,
        ..Default::default()
    };

    render_sections(&mut buffer, 23, &sections, &config);

    // Verify first section is rendered
    assert_eq!(buffer.get(1, 23).unwrap().char, 'N');
}

#[test]
fn test_render_sections_left_empty_filtered() {
    // Lines 102, 106: left-positioned but empty section should be filtered out
    let mut buffer = FrameBuffer::new(80, 24);

    let sections = vec![
        Section::new(SectionId::A, " NOR ", Style::new().bg(Color::Blue)),
        Section::empty(SectionId::B), // Empty left section, filtered at line 102
        Section::new(SectionId::C, " file.rs ", Style::new().bg(Color::Green)),
    ];

    let config = StatuslineRendererConfig {
        separator: StatuslineSeparator::PIPE,
        powerline_coloring: false,
        ..Default::default()
    };

    render_sections(&mut buffer, 23, &sections, &config);

    // A and C should be rendered, B should be filtered out
    assert_eq!(buffer.get(1, 23).unwrap().char, 'N'); // A section
}

#[test]
fn test_render_sections_empty_separator() {
    // Lines 117, 142: i > 0 but separator is empty (second condition false)
    let mut buffer = FrameBuffer::new(80, 24);

    let sections = vec![
        Section::new(SectionId::A, " A ", Style::default()),
        Section::new(SectionId::B, " B ", Style::default()),
        Section::new(SectionId::Y, " Y ", Style::default()),
        Section::new(SectionId::Z, " Z ", Style::default()),
    ];

    let config = StatuslineRendererConfig {
        separator: StatuslineSeparator {
            left: "",  // Empty separator for left sections
            right: "", // Empty separator for right sections
        },
        powerline_coloring: false,
        ..Default::default()
    };

    render_sections(&mut buffer, 23, &sections, &config);

    // Sections should render directly adjacent without separators
    assert_eq!(buffer.get(1, 23).unwrap().char, 'A');
    assert_eq!(buffer.get(4, 23).unwrap().char, 'B');
}

#[test]
fn test_render_sections_right_with_separators() {
    let mut buffer = FrameBuffer::new(80, 24);
    let style = Style::new().bg(Color::Blue);

    let sections = vec![
        Section::new(SectionId::Y, " utf-8 ", style.clone()),
        Section::new(SectionId::Z, " 42:15 ", style),
    ];

    let config = StatuslineRendererConfig {
        separator: StatuslineSeparator::PIPE,
        powerline_coloring: false,
        ..Default::default()
    };

    render_sections(&mut buffer, 23, &sections, &config);

    // Right sections should be rendered near the right edge
    let z_start = 80 - 14; // two 7-char sections
    // Verify content is present on the row
    let row_str: String = (z_start..80)
        .filter_map(|x| buffer.get(x, 23).map(|c| c.char))
        .collect();
    assert!(row_str.contains("42:15"), "Row should contain '42:15', got: {row_str}");
}

#[test]
fn test_render_sections_powerline_coloring() {
    let mut buffer = FrameBuffer::new(80, 24);
    let style_a = Style::new().bg(Color::Blue);
    let style_b = Style::new().bg(Color::Green);

    let sections = vec![
        Section::new(SectionId::A, " A ", style_a),
        Section::new(SectionId::B, " B ", style_b),
    ];

    let config = StatuslineRendererConfig {
        separator: StatuslineSeparator::POWERLINE_ARROW,
        powerline_coloring: true,
        ..Default::default()
    };

    render_sections(&mut buffer, 23, &sections, &config);

    // Just verify it renders without panicking
    assert_eq!(buffer.get(1, 23).unwrap().char, 'A');
}

#[test]
fn test_render_statusline_simple_narrow() {
    let mut buffer = FrameBuffer::new(20, 24);
    let mode_style = Style::new().bg(Color::Blue);
    let pos_style = Style::new().bg(Color::Green);
    let fill_style = Style::default();

    render_statusline_simple(
        &mut buffer,
        23,
        "NOR",
        "1:1",
        &mode_style,
        &pos_style,
        &fill_style,
    );

    // Mode at left
    assert_eq!(buffer.get(0, 23).unwrap().char, 'N');
    // Position at right
    assert_eq!(buffer.get(19, 23).unwrap().char, '1');
}

#[test]
fn test_separator_default() {
    let sep = StatuslineSeparator::default();
    assert_eq!(sep.left, StatuslineSeparator::POWERLINE_ARROW.left);
}

#[test]
fn test_statusline_renderer_config_default() {
    let config = StatuslineRendererConfig::default();
    assert!(config.powerline_coloring);
    assert_eq!(config.separator.left, StatuslineSeparator::POWERLINE_ARROW.left);
}

#[test]
fn test_truncate_sections_multiple_truncation() {
    let sections = vec![
        Section::with_priority(SectionId::A, "ABCDEFGHIJ", Style::default(), 100),
        Section::with_priority(SectionId::B, "1234567890", Style::default(), 50),
        Section::with_priority(SectionId::C, "XYZXYZXYZX", Style::default(), 10),
    ];

    // Total = 30, need to fit in 15
    let result = truncate_sections(&sections, 15);

    // Lowest priority (C) should be truncated first, then B
    let total_width: usize = result.iter().map(Section::display_width).sum();
    assert!(total_width <= 15, "Total width {total_width} should be <= 15");
}

#[test]
fn test_truncate_section_very_small_target() {
    let section = Section::with_priority(SectionId::A, "ABCDEFGHIJ", Style::default(), 100);
    let truncated = truncate_section(&section, 100);
    // When target_width <= TRUNCATION_WIDTH, should just be the indicator
    assert_eq!(truncated.text, "\u{2026}"); // "..."
}

#[test]
fn test_truncate_sections_small_section_skipped() {
    let sections = vec![
        Section::with_priority(SectionId::A, "X", Style::default(), 10), // Too small to truncate
        Section::with_priority(SectionId::B, "ABCDEFGHIJ", Style::default(), 50),
    ];

    // Total = 11, need to fit in 5
    let result = truncate_sections(&sections, 5);
    // Small section should be kept, larger truncated
    assert_eq!(result[0].text, "X");
}

#[test]
fn test_render_section_content_at_max_width() {
    let mut buffer = FrameBuffer::new(5, 1);
    let section = Section::new(SectionId::A, "Hello World", Style::default());

    let next_x = render_section_content(&mut buffer, 0, 0, &section, 5);
    // Should stop at max_width
    assert_eq!(next_x, 5);
}

#[test]
fn test_separator_round_constants() {
    assert_eq!(StatuslineSeparator::POWERLINE_ROUND.left, "\u{e0b4}");
    assert_eq!(StatuslineSeparator::POWERLINE_ROUND.right, "\u{e0b6}");
}

#[test]
fn test_render_separator_at_buffer_edge() {
    // Buffer width 3, separator starts at position 2 - separator char breaks at edge
    let mut buffer = FrameBuffer::new(3, 1);
    let style = Style::new().bg(Color::Blue);

    let sections = vec![
        Section::new(SectionId::A, "AB", style.clone()),
        Section::new(SectionId::B, "CD", style),
    ];

    let config = StatuslineRendererConfig {
        separator: StatuslineSeparator::PIPE,
        powerline_coloring: false,
        ..Default::default()
    };

    // This exercises the `if pos >= buffer.width() { break; }` path (line 209)
    // because after rendering "AB" (x=2), the separator "|" starts at x=2,
    // and then "CD" starts at x=3 which is >= width(3).
    render_sections(&mut buffer, 0, &sections, &config);

    assert_eq!(buffer.get(0, 0).unwrap().char, 'A');
    assert_eq!(buffer.get(1, 0).unwrap().char, 'B');
}

#[test]
fn test_render_statusline_simple_mode_exceeds_width() {
    // Mode text longer than buffer width - exercises `if x >= width { break; }` at line 252
    // Use empty position to avoid overwriting mode chars.
    let mut buffer = FrameBuffer::new(3, 1);
    let mode_style = Style::new().bg(Color::Blue);
    let pos_style = Style::new().bg(Color::Green);
    let fill_style = Style::default();

    render_statusline_simple(
        &mut buffer,
        0,
        "NORMAL",
        "",
        &mode_style,
        &pos_style,
        &fill_style,
    );

    // Only first 3 chars fit, the rest are truncated by the break
    assert_eq!(buffer.get(0, 0).unwrap().char, 'N');
    assert_eq!(buffer.get(1, 0).unwrap().char, 'O');
    assert_eq!(buffer.get(2, 0).unwrap().char, 'R');
}

#[test]
fn test_render_separator_break_on_width_exceeded() {
    // Covers line 209: break in render_separator when pos >= buffer.width()
    // Use a buffer just wide enough for "AB" (2 chars), then the separator
    // character would need to start at x=2 which equals the width.
    let mut buffer = FrameBuffer::new(2, 1);

    // Render separator directly to test the break condition
    let style = Style::new().bg(Color::Blue);
    let pos = render_separator(
        &mut buffer,
        2, // Start at the edge
        0,
        Some(&style),
        Some(&style),
        "|", // Single char separator
        false,
    );
    // pos should remain 2 since the separator char couldn't be rendered
    assert_eq!(pos, 2);
}

#[test]
fn test_render_statusline_simple_position_exceeds_width() {
    // Position starts near right edge and position text extends past buffer
    // This exercises `if x >= width { break; }` at line 267
    let mut buffer = FrameBuffer::new(5, 1);
    let mode_style = Style::new().bg(Color::Blue);
    let pos_style = Style::new().bg(Color::Green);
    let fill_style = Style::default();

    // Position "123456" is 6 chars, but buffer is only 5 wide.
    // pos_start = 5 - 6 = 0 (saturating_sub), so it starts at 0.
    // Rendering position chars: at x=5 the break triggers.
    render_statusline_simple(
        &mut buffer,
        0,
        "",
        "123456",
        &mode_style,
        &pos_style,
        &fill_style,
    );

    assert_eq!(buffer.get(0, 0).unwrap().char, '1');
    assert_eq!(buffer.get(4, 0).unwrap().char, '5');
}

#[test]
fn test_render_sections_right_empty_filtered() {
    // Line 106: s.position() == Right (true) && !s.is_empty() (false)
    // An empty right section should be filtered out
    let mut buffer = FrameBuffer::new(80, 24);

    let sections = vec![
        Section::new(SectionId::Y, " utf-8 ", Style::new().bg(Color::Blue)),
        Section::empty(SectionId::Z), // Empty right section, filtered at line 106
    ];

    let config = StatuslineRendererConfig {
        separator: StatuslineSeparator::PIPE,
        powerline_coloring: false,
        ..Default::default()
    };

    render_sections(&mut buffer, 23, &sections, &config);

    // Y should be rendered near the right edge, Z should be filtered out
    let y_start = 80 - 7; // " utf-8 " = 7 chars
    assert_eq!(buffer.get(y_start + 1, 23).unwrap().char, 'u');
}
