use super::*;

#[test]
fn test_display_width_ascii() {
    assert_eq!(display_width("Hello"), 5);
    assert_eq!(display_width(""), 0);
    assert_eq!(display_width("  "), 2);
}

#[test]
fn test_display_width_cjk() {
    // CJK characters are typically width 2
    assert_eq!(display_width("你好"), 4);
    assert_eq!(display_width("日本語"), 6);
}

#[test]
fn test_truncate_end_no_truncation() {
    assert_eq!(truncate_end("Hi", 10), "Hi");
    assert_eq!(truncate_end("Hello", 5), "Hello");
}

#[test]
fn test_truncate_end_truncation() {
    assert_eq!(truncate_end("Hello, World!", 8), "Hello...");
    assert_eq!(truncate_end("Hello, World!", 5), "He...");
}

#[test]
fn test_truncate_end_very_short() {
    assert_eq!(truncate_end("Hello", 3), "...");
    assert_eq!(truncate_end("Hello", 2), "..");
    assert_eq!(truncate_end("Hello", 1), ".");
}

#[test]
fn test_truncate_start_no_truncation() {
    assert_eq!(truncate_start("Hi", 10), "Hi");
}

#[test]
fn test_truncate_start_truncation() {
    let result = truncate_start("/very/long/path/file.rs", 12);
    assert!(result.starts_with("..."));
    assert_eq!(display_width(&result), 12);
}

#[test]
fn test_align_left() {
    assert_eq!(align("Hi", 6, Alignment::Left), "Hi    ");
}

#[test]
fn test_align_right() {
    assert_eq!(align("Hi", 6, Alignment::Right), "    Hi");
}

#[test]
fn test_align_center() {
    assert_eq!(align("Hi", 6, Alignment::Center), "  Hi  ");
    assert_eq!(align("Hi", 7, Alignment::Center), "  Hi   "); // Odd padding
}

#[test]
fn test_align_no_change() {
    assert_eq!(align("Hello!", 6, Alignment::Left), "Hello!");
    assert_eq!(align("Hello!", 4, Alignment::Left), "Hello!");
}

#[test]
fn test_pad_left() {
    assert_eq!(pad_left("42", 5, '0'), "00042");
    assert_eq!(pad_left("42", 5, ' '), "   42");
    assert_eq!(pad_left("Hello", 3, ' '), "Hello"); // Already wider
}

#[test]
fn test_pad_right() {
    assert_eq!(pad_right("Hi", 5, '.'), "Hi...");
    assert_eq!(pad_right("Hi", 5, ' '), "Hi   ");
    assert_eq!(pad_right("Hello", 3, ' '), "Hello"); // Already wider
}

#[test]
fn test_wrap_text_simple() {
    let lines = wrap_text("Hello World", 20);
    assert_eq!(lines, vec!["Hello World"]);
}

#[test]
fn test_wrap_text_multiple_lines() {
    let lines = wrap_text("Hello World", 6);
    assert_eq!(lines, vec!["Hello", "World"]);
}

#[test]
fn test_wrap_text_long_word() {
    let lines = wrap_text("Supercalifragilisticexpialidocious", 10);
    assert!(lines.len() > 1);
    for line in &lines {
        assert!(display_width(line) <= 10);
    }
}

#[test]
fn test_wrap_text_empty() {
    let lines = wrap_text("", 10);
    assert!(lines.is_empty());
}

#[test]
fn test_wrap_text_zero_width() {
    let lines = wrap_text("Hello", 0);
    assert!(lines.is_empty());
}

#[test]
fn test_truncate_start_very_short() {
    // Line 95: truncate_start with max_width <= 3
    assert_eq!(truncate_start("Hello, World!", 3), "...");
    assert_eq!(truncate_start("Hello, World!", 2), "..");
    assert_eq!(truncate_start("Hello, World!", 1), ".");
    assert_eq!(truncate_start("Hello, World!", 0), "");
}

#[test]
fn test_pad_left_wide_fill_char_trimming() {
    // Lines 193-204: pad_left with a wide (CJK) fill character that
    // causes the result to exceed the target width.
    // "X" has width 1. Padding to width 5 means 4 columns of fill.
    // Using a CJK fill char (width 2): div_ceil(4, 2) = 2 fill chars = 4 columns.
    // Total = 4 + 1 = 5. Exact fit, no trimming needed.
    //
    // To trigger trimming: pad "X" (width 1) to width 4.
    // Padding needed = 3. fill_count = div_ceil(3, 2) = 2.
    // 2 CJK fill chars = 4 columns + "X" = 5 columns > 4. Triggers trim.
    let result = pad_left("X", 4, '\u{4e00}'); // U+4E00 = '一' (width 2)
    assert!(display_width(&result) <= 4);
    // The trimming removes from the beginning, so "X" should still be at the end
    assert!(result.ends_with('X'));
}

#[test]
fn test_pad_left_wide_fill_odd_padding() {
    // Another case: pad "ab" (width 2) to width 5 with CJK fill.
    // Padding = 3. fill_count = div_ceil(3, 2) = 2.
    // 2 CJK fills = 4 + "ab" = 6 > 5. Triggers trim path.
    let result = pad_left("ab", 5, '\u{4e16}'); // U+4E16 = '世' (width 2)
    assert!(display_width(&result) <= 5);
    assert!(result.ends_with("ab"));
}

#[test]
fn test_pad_right_wide_fill_char_trimming() {
    // Lines 239-249: pad_right with a wide (CJK) fill character that
    // causes the result to exceed the target width.
    // "X" (width 1) padded to width 4 with CJK fill (width 2).
    // Padding = 3. fill_count = div_ceil(3, 2) = 2.
    // "X" + 2 CJK = 1 + 4 = 5 > 4. Triggers trim.
    let result = pad_right("X", 4, '\u{4e00}'); // U+4E00 = '一' (width 2)
    assert!(display_width(&result) <= 4);
    // The text should still start with "X"
    assert!(result.starts_with('X'));
}

#[test]
fn test_pad_right_wide_fill_odd_padding() {
    // pad "ab" (width 2) to width 5 with CJK fill.
    // Padding = 3. fill_count = div_ceil(3, 2) = 2.
    // "ab" + 2 CJK = 2 + 4 = 6 > 5. Triggers trim path.
    let result = pad_right("ab", 5, '\u{4e16}'); // U+4E16 = '世' (width 2)
    assert!(display_width(&result) <= 5);
    assert!(result.starts_with("ab"));
}

#[test]
fn test_wrap_text_long_word_after_line_break() {
    // Lines 309-319: wrap_text where a too-long word appears after
    // the current line already has content, forcing a line break AND
    // then the word itself is too long for max_width.
    // "Hi" fits on first line (width 2 <= 4).
    // "你好世界" (width 8) doesn't fit with space (2+1+8=11 > 4).
    // So we push "Hi", then try to fit "你好世界" which is > 4.
    // This enters the break-long-word path at lines 309-319.
    let lines = wrap_text("Hi 你好世界", 4);
    assert_eq!(lines[0], "Hi");
    // Each CJK char is width 2, max_width is 4, so 2 chars per line
    assert!(lines.len() >= 3);
    for line in &lines {
        assert!(display_width(line) <= 4);
    }
}

#[test]
fn test_wrap_text_wide_char_boundary_break() {
    // Test that wide chars at the exact boundary cause correct wrapping.
    // max_width = 3: one CJK char (width 2) fits, but two (width 4) don't.
    // "你好世界" as a single word on a new line (after another word).
    let lines = wrap_text("ab 你好世界", 3);
    assert_eq!(lines[0], "ab");
    // Each CJK char takes width 2, only 1 fits per line of width 3
    for line in &lines[1..] {
        assert!(display_width(line) <= 3);
    }
}
