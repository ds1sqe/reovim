use super::*;

#[test]
fn test_identity_no_decorations() {
    let content = "Hello, World!";
    let result = apply_conceals(content, 0, &[]);

    assert_eq!(result.text, content);
    assert_eq!(result.col_mapping.len(), content.len() + 1);
}

#[test]
fn test_simple_conceal() {
    let content = "[text](url)";
    let decorations = [&Decoration::conceal(
        Span::line(0, 0, content.len() as u32),
        "[link]",
        None,
    )];

    let result = apply_conceals(content, 0, &decorations);

    assert_eq!(result.text, "[link]");
}

#[test]
fn test_simple_hide() {
    let content = "Hello hidden World";
    // Hide "hidden " (positions 6-13)
    let decorations = [&Decoration::hide(Span::line(0, 6, 13))];

    let result = apply_conceals(content, 0, &decorations);

    assert_eq!(result.text, "Hello World");
}

#[test]
fn test_display_to_source_mapping() {
    let content = "abc123def";
    // Replace "123" with "X"
    let decorations = [&Decoration::conceal(Span::line(0, 3, 6), "X", None)];

    let result = apply_conceals(content, 0, &decorations);

    assert_eq!(result.text, "abcXdef");

    // Position 0-2 map to themselves
    assert_eq!(display_to_source_col(&result, 0), 0);
    assert_eq!(display_to_source_col(&result, 2), 2);

    // Position 3 (X) maps to start of concealed region
    assert_eq!(display_to_source_col(&result, 3), 3);

    // Position 4 (d) maps to 6 (after concealed region)
    assert_eq!(display_to_source_col(&result, 4), 6);
}

#[test]
fn test_source_to_display_mapping() {
    let content = "abc123def";
    // Replace "123" with "X"
    let decorations = [&Decoration::conceal(Span::line(0, 3, 6), "X", None)];

    let result = apply_conceals(content, 0, &decorations);

    // Source 0-2 map to themselves
    assert_eq!(source_to_display_col(&result, 0), 0);
    assert_eq!(source_to_display_col(&result, 2), 2);

    // Source 3-5 (concealed region) maps to 3 (X position)
    assert_eq!(source_to_display_col(&result, 3), 3);

    // Source 6 (d) maps to 4
    assert_eq!(source_to_display_col(&result, 6), 4);
}

#[test]
fn test_concealed_line_display_width() {
    let line = ConcealedLine::identity("hello");
    assert_eq!(line.display_width(), 5);

    let empty = ConcealedLine::identity("");
    assert_eq!(empty.display_width(), 0);
}

#[test]
fn test_concealed_line_source_width() {
    let line = ConcealedLine::identity("hello");
    assert_eq!(line.source_width(), 5);

    // When concealing, source_width reflects original content length
    let content = "abc123def";
    let decorations = [&Decoration::conceal(Span::line(0, 3, 6), "X", None)];
    let result = apply_conceals(content, 0, &decorations);
    assert_eq!(result.source_width(), 9); // original "abc123def" = 9
}

#[test]
fn test_concealed_line_source_width_empty() {
    let line = ConcealedLine {
        text: String::new(),
        col_mapping: Vec::new(),
        styles: Vec::new(),
    };
    assert_eq!(line.source_width(), 0);
}

#[test]
fn test_apply_conceals_non_matching_decorations() {
    // Decorations that don't match the type (not Conceal or Hide)
    let content = "hello world";
    let inline_style = Decoration::inline_style(Span::line(0, 0, 5), Style::default());
    let line_bg = Decoration::line_background(0, 0, Style::default());
    let decorations = [&inline_style, &line_bg];

    let result = apply_conceals(content, 0, &decorations);
    assert_eq!(result.text, "hello world"); // No concealment applied
}

#[test]
fn test_apply_conceals_wrong_line() {
    // Conceal on line 5, but we render line 0
    let content = "hello world";
    let conceal = Decoration::conceal(Span::line(5, 0, 5), "X", None);
    let decorations = [&conceal];

    let result = apply_conceals(content, 0, &decorations);
    assert_eq!(result.text, "hello world"); // No concealment on this line
}

#[test]
fn test_apply_conceals_empty_regions_after_filter() {
    // Decorations exist but none match the current line after filtering
    let content = "hello world";
    let conceal = Decoration::conceal(Span::line(3, 0, 5), "X", None);
    let decorations = [&conceal];

    // Line 0 doesn't match span on line 3, so regions will be empty
    let result = apply_conceals(content, 0, &decorations);
    assert_eq!(result.text, "hello world");
}

#[test]
fn test_apply_conceals_overlapping_regions() {
    let content = "abcdefghij"; // 10 chars
    // Two overlapping conceals: 0-5 and 3-8
    let c1 = Decoration::conceal(Span::line(0, 0, 5), "X", None);
    let c2 = Decoration::conceal(Span::line(0, 3, 8), "Y", None);
    let decorations = [&c1, &c2];

    let result = apply_conceals(content, 0, &decorations);
    // First conceal replaces 0-5 with "X", source_col advances to 5
    // Second conceal starts at 3 which is < 5 (source_col), so it's skipped
    // Remaining: "fghij" (5-10)
    assert_eq!(result.text, "Xfghij");
}

#[test]
fn test_multi_line_span_first_line() {
    // Multi-line span: line 0 col 5 to line 2 col 3
    let content = "hello world"; // line 0
    let span = Span::new(0, 5, 2, 3);
    let conceal = Decoration::Conceal {
        span,
        replacement: "X".to_string(),
        style: None,
    };
    let decorations = [&conceal];

    let result = apply_conceals(content, 0, &decorations);
    // First line of multi-line span: from start_col(5) to end of line
    assert_eq!(result.text, "helloX");
}

#[test]
fn test_multi_line_span_last_line() {
    let content = "hello world"; // line 2
    let span = Span::new(0, 5, 2, 5);
    let conceal = Decoration::Conceal {
        span,
        replacement: "Y".to_string(),
        style: None,
    };
    let decorations = [&conceal];

    // Last line: from 0 to end_col(5)
    let result = apply_conceals(content, 2, &decorations);
    assert_eq!(result.text, "Y world");
}

#[test]
fn test_multi_line_span_middle_line() {
    let content = "hello world"; // line 1 (middle)
    let span = Span::new(0, 5, 2, 3);
    let conceal = Decoration::Conceal {
        span,
        replacement: "Z".to_string(),
        style: None,
    };
    let decorations = [&conceal];

    // Middle line: entire line (0 to line_len)
    let result = apply_conceals(content, 1, &decorations);
    assert_eq!(result.text, "Z");
}

#[test]
fn test_multi_line_hide_middle_line() {
    let content = "hello world"; // line 1 (middle)
    let span = Span::new(0, 5, 2, 3);
    let hide = Decoration::Hide { span };
    let decorations = [&hide];

    // Hide on middle line: entire line hidden
    let result = apply_conceals(content, 1, &decorations);
    assert_eq!(result.text, "");
}

#[test]
fn test_source_to_display_col_beyond_end() {
    let content = "abc";
    let result = ConcealedLine::identity(content);

    // Source col beyond end should return text.len()
    assert_eq!(source_to_display_col(&result, 100), 3);
}

#[test]
fn test_display_to_source_col_beyond_end() {
    let content = "abc";
    let result = ConcealedLine::identity(content);

    // Display col beyond end should return last mapping
    assert_eq!(display_to_source_col(&result, 100), 3);
}

#[test]
fn test_hide_span_different_line() {
    // Hide span on a different line should not produce a region (line 93 guard false)
    let content = "abcdef";
    let hide = Decoration::hide(Span::line(5, 0, 3)); // line 5, but we render line 0
    let decorations = [&hide];

    let result = apply_conceals(content, 0, &decorations);
    // No concealment should apply since the span is on line 5
    assert_eq!(result.text, "abcdef");
}

#[test]
fn test_conceal_with_style() {
    let content = "abc123def";
    let style = Style::new().fg(crate::Color::Red);
    let conceal = Decoration::conceal(Span::line(0, 3, 6), "XX", Some(style.clone()));
    let decorations = [&conceal];

    let result = apply_conceals(content, 0, &decorations);
    assert_eq!(result.text, "abcXXdef");
    // Check that style is set for the replacement chars
    assert_eq!(result.styles[3], Some(style.clone()));
    assert_eq!(result.styles[4], Some(style));
}
