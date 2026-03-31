use crate::display_lines::*;

#[test]
fn test_display_line_count_empty() {
    assert_eq!(display_line_count("", 80), 1);
}

#[test]
fn test_display_line_count_short() {
    assert_eq!(display_line_count("hello", 80), 1);
}

#[test]
fn test_display_line_count_exact() {
    let line = "x".repeat(80);
    assert_eq!(display_line_count(&line, 80), 1);
}

#[test]
fn test_display_line_count_wrap() {
    let line = "x".repeat(81);
    assert_eq!(display_line_count(&line, 80), 2);
}

#[test]
fn test_display_line_count_multiple_wraps() {
    let line = "x".repeat(241);
    assert_eq!(display_line_count(&line, 80), 4);
}

#[test]
fn test_display_line_count_zero_width() {
    assert_eq!(display_line_count("hello", 0), 1);
}

#[test]
fn test_display_position_first_line() {
    assert_eq!(display_position(5, 80), (0, 5));
    assert_eq!(display_position(79, 80), (0, 79));
}

#[test]
fn test_display_position_wrapped() {
    assert_eq!(display_position(80, 80), (1, 0));
    assert_eq!(display_position(85, 80), (1, 5));
    assert_eq!(display_position(160, 80), (2, 0));
}

#[test]
fn test_display_position_zero_width() {
    assert_eq!(display_position(50, 0), (0, 50));
}

#[test]
fn test_buffer_column_first_line() {
    assert_eq!(buffer_column(0, 5, 80), 5);
}

#[test]
fn test_buffer_column_wrapped() {
    assert_eq!(buffer_column(1, 5, 80), 85);
    assert_eq!(buffer_column(2, 0, 80), 160);
}

#[test]
fn test_roundtrip() {
    for col in [0, 5, 79, 80, 85, 160, 165] {
        let (dl, dc) = display_position(col, 80);
        assert_eq!(buffer_column(dl, dc, 80), col);
    }
}

// ========================================================================
// Tab-aware function tests
// ========================================================================

#[test]
fn test_display_width_with_tabs_basic() {
    // "a\tb" with tabstop=4: 'a'(1) + tab(3 to col 4) + 'b'(1) = 5
    assert_eq!(display_width_with_tabs("a\tb", 4), 5);
}

#[test]
fn test_display_width_with_tabs_double_tab() {
    // "\t\t" with tabstop=8: tab(8) + tab(8) = 16
    assert_eq!(display_width_with_tabs("\t\t", 8), 16);
}

#[test]
fn test_display_width_with_tabs_at_start() {
    // Tab at column 0 with tabstop=4 takes full 4 columns
    assert_eq!(display_width_with_tabs("\t", 4), 4);
    assert_eq!(display_width_with_tabs("\tx", 4), 5);
}

#[test]
fn test_display_width_with_tabs_contextual() {
    // Tab at column 2 with tabstop=4 takes 2 columns (to reach 4)
    assert_eq!(display_width_with_tabs("ab\t", 4), 4);
    // Tab at column 3 with tabstop=4 takes 1 column (to reach 4)
    assert_eq!(display_width_with_tabs("abc\t", 4), 4);
}

#[test]
fn test_display_width_with_tabs_only_char() {
    // Single tab is the only character
    assert_eq!(display_width_with_tabs("\t", 4), 4);
    assert_eq!(display_width_with_tabs("\t", 8), 8);
    assert_eq!(display_width_with_tabs("\t", 2), 2);
}

#[test]
fn test_display_width_with_tabs_consecutive() {
    // Multiple consecutive tabs at different positions
    assert_eq!(display_width_with_tabs("\t\t\t", 4), 12); // 4 + 4 + 4
    assert_eq!(display_width_with_tabs("x\t\t", 4), 8); // 1 + 3 + 4
}

#[test]
fn test_display_width_with_tabs_mixed() {
    // Mixed tabs and spaces
    assert_eq!(display_width_with_tabs("  \t", 4), 4); // 2 + 2
    assert_eq!(display_width_with_tabs("\t  ", 4), 6); // 4 + 2
}

#[test]
fn test_display_width_with_tabs_various_tabstops() {
    let line = "x\ty";
    // tabstop=1: each tab advances by 1
    assert_eq!(display_width_with_tabs(line, 1), 3); // 1 + 1 + 1
    // tabstop=2: x(1) + tab(1 to reach col 2) + y(1) = 3
    assert_eq!(display_width_with_tabs(line, 2), 3);
    // tabstop=4: x(1) + tab(3 to reach col 4) + y(1) = 5
    assert_eq!(display_width_with_tabs(line, 4), 5);
    // tabstop=8: x(1) + tab(7 to reach col 8) + y(1) = 9
    assert_eq!(display_width_with_tabs(line, 8), 9);
    // tabstop=16: x(1) + tab(15 to reach col 16) + y(1) = 17
    assert_eq!(display_width_with_tabs(line, 16), 17);
}

#[test]
fn test_display_width_with_tabs_zero_tabstop() {
    // Zero tabstop should be treated as 1 (prevent division by zero)
    assert_eq!(display_width_with_tabs("a\tb", 0), 3);
}

#[test]
fn test_display_width_with_tabs_empty() {
    assert_eq!(display_width_with_tabs("", 4), 0);
}

#[test]
fn test_display_line_count_with_tabs() {
    // Line with tabs that causes wrapping
    let line = "\t\t\t"; // 12 columns with tabstop=4
    assert_eq!(display_line_count_with_tabs(line, 10, 4), 2);
    assert_eq!(display_line_count_with_tabs(line, 12, 4), 1);
    assert_eq!(display_line_count_with_tabs(line, 6, 4), 2);
}

#[test]
fn test_display_col_from_buffer_col() {
    let line = "a\tb";
    // Buffer col 0 (before 'a') = display col 0
    assert_eq!(display_col_from_buffer_col(line, 0, 4), 0);
    // Buffer col 1 (after 'a', before tab) = display col 1
    assert_eq!(display_col_from_buffer_col(line, 1, 4), 1);
    // Buffer col 2 (after tab, before 'b') = display col 4
    assert_eq!(display_col_from_buffer_col(line, 2, 4), 4);
    // Buffer col 3 (after 'b') = display col 5
    assert_eq!(display_col_from_buffer_col(line, 3, 4), 5);
}

#[test]
fn test_buffer_col_from_display_col() {
    let line = "a\tb";
    // Display col 0 = buffer col 0 (at 'a')
    assert_eq!(buffer_col_from_display_col(line, 0, 4), 0);
    // Display col 1 = buffer col 1 (after 'a', at start of tab)
    assert_eq!(buffer_col_from_display_col(line, 1, 4), 1);
    // Display col 2,3 = buffer col 1 (inside tab, can't land here)
    assert_eq!(buffer_col_from_display_col(line, 2, 4), 1);
    assert_eq!(buffer_col_from_display_col(line, 3, 4), 1);
    // Display col 4 = buffer col 2 (at 'b')
    assert_eq!(buffer_col_from_display_col(line, 4, 4), 2);
    // Display col 5 = buffer col 3 (after 'b')
    assert_eq!(buffer_col_from_display_col(line, 5, 4), 3);
}

#[test]
fn test_display_position_with_tabs() {
    let line = "a\tb";
    // Buffer col 2 (at 'b') = display col 4, on display line 0 for 80-col terminal
    assert_eq!(display_position_with_tabs(line, 2, 80, 4), (0, 4));
}

#[test]
fn test_display_position_with_tabs_wrapped() {
    // Create a line that wraps with tabs
    let line = "\t\t\t"; // 12 display columns with tabstop=4
    // Buffer col 0 = display col 0, display line 0
    assert_eq!(display_position_with_tabs(line, 0, 10, 4), (0, 0));
    // Buffer col 1 = display col 4 (after first tab), still line 0
    assert_eq!(display_position_with_tabs(line, 1, 10, 4), (0, 4));
    // Buffer col 2 = display col 8 (after second tab), still line 0
    assert_eq!(display_position_with_tabs(line, 2, 10, 4), (0, 8));
    // Buffer col 3 = display col 12, wraps to line 1, col 2
    assert_eq!(display_position_with_tabs(line, 3, 10, 4), (1, 2));
}

#[test]
fn test_tab_roundtrip() {
    // Test that buffer_col -> display_col -> buffer_col round-trips
    let line = "abc\tdef\tghi";
    for buffer_col in 0..=line.chars().count() {
        let display_col = display_col_from_buffer_col(line, buffer_col, 4);
        let back = buffer_col_from_display_col(line, display_col, 4);
        assert_eq!(back, buffer_col, "Roundtrip failed for buffer_col={buffer_col}");
    }
}

#[test]
fn test_tab_at_end_of_line() {
    let line = "hello\t";
    // "hello" = 5 cols, tab at col 5 with tabstop=4 advances to col 8 (width 3)
    assert_eq!(display_width_with_tabs(line, 4), 8);
}

// ========================================================================
// Unicode-aware function tests (CJK + tabs)
// ========================================================================

#[test]
fn test_char_display_width_ascii() {
    assert_eq!(char_display_width('a'), 1);
    assert_eq!(char_display_width('Z'), 1);
    assert_eq!(char_display_width(' '), 1);
    assert_eq!(char_display_width('!'), 1);
}

#[test]
fn test_char_display_width_cjk() {
    // CJK ideographs are double-width
    assert_eq!(char_display_width('\u{4E2D}'), 2);
    assert_eq!(char_display_width('\u{65E5}'), 2);
    assert_eq!(char_display_width('\u{672C}'), 2);
    assert_eq!(char_display_width('\u{8A9E}'), 2);
}

#[test]
fn test_char_display_width_fullwidth() {
    // Fullwidth ASCII variants
    assert_eq!(char_display_width('\u{FF21}'), 2); // Fullwidth A
    assert_eq!(char_display_width('\u{FF11}'), 2); // Fullwidth 1
}

#[test]
fn test_char_display_width_combining() {
    // Combining marks have width 0
    assert_eq!(char_display_width('\u{0301}'), 0); // Combining acute accent
    assert_eq!(char_display_width('\u{0308}'), 0); // Combining diaeresis
}

#[test]
fn test_display_width_unicode_ascii() {
    assert_eq!(display_width_unicode("Hello", 4), 5);
    assert_eq!(display_width_unicode("Hello World", 4), 11);
}

#[test]
fn test_display_width_unicode_cjk() {
    // 5 CJK characters = 10 display columns
    assert_eq!(display_width_unicode("\u{4E2D}\u{6587}\u{6D4B}\u{8BD5}\u{5B57}", 4), 10);
    // 3 CJK = 6 columns
    assert_eq!(display_width_unicode("\u{65E5}\u{672C}\u{8A9E}", 4), 6);
}

#[test]
fn test_display_width_unicode_mixed() {
    // "Hello" + 2 CJK = 5 + 4 = 9 columns
    assert_eq!(display_width_unicode("Hello\u{4E2D}\u{6587}", 4), 9);
    // "a" + CJK + "b" = 1 + 2 + 1 = 4 columns
    assert_eq!(display_width_unicode("a\u{4E2D}b", 4), 4);
}

#[test]
fn test_display_width_unicode_with_tabs() {
    // "a\t" + CJK with tabstop=4: a(1) + tab(3) + CJK(2) = 6
    assert_eq!(display_width_unicode("a\t\u{4E2D}", 4), 6);
    // "\t" + CJK with tabstop=4: tab(4) + CJK(2) = 6
    assert_eq!(display_width_unicode("\t\u{4E2D}", 4), 6);
}

#[test]
fn test_display_width_unicode_combining() {
    // "e" as e + combining accent = 1 + 0 = 1
    assert_eq!(display_width_unicode("e\u{0301}", 4), 1);
    // "nai" + combining diaeresis + "ve"
    assert_eq!(display_width_unicode("nai\u{0308}ve", 4), 5);
}

#[test]
fn test_display_width_unicode_tabs_and_cjk_integration() {
    // Critical integration test: tabs + CJK in same line
    // CJK(2) + tab(2 to reach 4) + CJK(2) = 6
    assert_eq!(display_width_unicode("\u{4E2D}\t\u{65E5}", 4), 6);
    // a(1) + CJK(2) + tab(1 to reach 4) + b(1) = 5
    assert_eq!(display_width_unicode("a\u{4E2D}\tb", 4), 5);
}

#[test]
fn test_display_col_from_buffer_col_unicode_cjk() {
    let line = "a\u{4E2D}b";
    // Buffer col 0 = display col 0
    assert_eq!(display_col_from_buffer_col_unicode(line, 0, 4), 0);
    // Buffer col 1 (after 'a') = display col 1
    assert_eq!(display_col_from_buffer_col_unicode(line, 1, 4), 1);
    // Buffer col 2 (after CJK) = display col 3 (1 + 2)
    assert_eq!(display_col_from_buffer_col_unicode(line, 2, 4), 3);
    // Buffer col 3 (after 'b') = display col 4
    assert_eq!(display_col_from_buffer_col_unicode(line, 3, 4), 4);
}

#[test]
fn test_buffer_col_from_display_col_unicode_cjk() {
    let line = "a\u{4E2D}b";
    // Display col 0 = buffer col 0
    assert_eq!(buffer_col_from_display_col_unicode(line, 0, 4), 0);
    // Display col 1 = buffer col 1 (at CJK)
    assert_eq!(buffer_col_from_display_col_unicode(line, 1, 4), 1);
    // Display col 2 = buffer col 1 (inside CJK, can't land here)
    assert_eq!(buffer_col_from_display_col_unicode(line, 2, 4), 1);
    // Display col 3 = buffer col 2 (at 'b')
    assert_eq!(buffer_col_from_display_col_unicode(line, 3, 4), 2);
    // Display col 4 = buffer col 3 (after 'b')
    assert_eq!(buffer_col_from_display_col_unicode(line, 4, 4), 3);
}

#[test]
fn test_display_position_unicode_cjk() {
    let line = "a\u{4E2D}b";
    // Buffer col 2 (at 'b') = display col 3
    assert_eq!(display_position_unicode(line, 2, 80, 4), (0, 3));
}

#[test]
fn test_unicode_roundtrip_cjk() {
    let line = "Hello\u{4E2D}\u{6587}World";
    for buffer_col in 0..=line.chars().count() {
        let display_col = display_col_from_buffer_col_unicode(line, buffer_col, 4);
        let back = buffer_col_from_display_col_unicode(line, display_col, 4);
        assert_eq!(back, buffer_col, "Roundtrip failed for buffer_col={buffer_col}");
    }
}

#[test]
fn test_unicode_roundtrip_tabs_and_cjk() {
    let line = "a\t\u{4E2D}\tb";
    for buffer_col in 0..=line.chars().count() {
        let display_col = display_col_from_buffer_col_unicode(line, buffer_col, 4);
        let back = buffer_col_from_display_col_unicode(line, display_col, 4);
        assert_eq!(back, buffer_col, "Roundtrip failed for buffer_col={buffer_col}");
    }
}

#[test]
fn test_cursor_never_lands_inside_wide_char() {
    let line = "\u{4E2D}"; // Single CJK char, 2 columns wide
    // Display col 0 = buffer col 0 (at start of CJK)
    assert_eq!(buffer_col_from_display_col_unicode(line, 0, 4), 0);
    // Display col 1 = buffer col 0 (inside CJK, stays at start)
    assert_eq!(buffer_col_from_display_col_unicode(line, 1, 4), 0);
    // Display col 2 = buffer col 1 (after CJK)
    assert_eq!(buffer_col_from_display_col_unicode(line, 2, 4), 1);
}

#[test]
fn test_display_line_count_unicode() {
    // CJK text that wraps
    let line = "\u{4E2D}\u{6587}\u{4E2D}\u{6587}\u{4E2D}"; // 5 chars * 2 = 10 columns
    assert_eq!(display_line_count_unicode(line, 8, 4), 2); // 10 / 8 = 2
    assert_eq!(display_line_count_unicode(line, 10, 4), 1); // 10 / 10 = 1
    assert_eq!(display_line_count_unicode(line, 4, 4), 3); // 10 / 4 = 3
}

// ========================================================================
// Edge case tests
// ========================================================================

#[test]
fn test_char_display_width_tab() {
    // Tab is reported as 1 by char_display_width (contextual width
    // should use char_display_width_at instead)
    assert_eq!(char_display_width('\t'), 1);
}

#[test]
fn test_char_display_width_at_tab() {
    // Tab width depends on position
    assert_eq!(char_display_width_at('\t', 0, 4), 4);
    assert_eq!(char_display_width_at('\t', 1, 4), 3);
    assert_eq!(char_display_width_at('\t', 2, 4), 2);
    assert_eq!(char_display_width_at('\t', 3, 4), 1);
    assert_eq!(char_display_width_at('\t', 4, 4), 4);
    // Zero tabstop treated as 1
    assert_eq!(char_display_width_at('\t', 0, 0), 1);
}

#[test]
fn test_display_line_count_with_tabs_zero_width() {
    // Terminal width 0 always returns 1
    assert_eq!(display_line_count_with_tabs("hello\tworld", 0, 4), 1);
}

#[test]
fn test_display_line_count_with_tabs_empty_line() {
    // Empty line returns 1 (display_width_with_tabs returns 0, so width == 0 branch)
    assert_eq!(display_line_count_with_tabs("", 80, 4), 1);
}

#[test]
fn test_display_position_with_tabs_zero_width() {
    // Terminal width 0: returns (0, display_col)
    let line = "a\tb";
    let (dl, dc) = display_position_with_tabs(line, 2, 0, 4);
    assert_eq!(dl, 0);
    assert_eq!(dc, 4); // 'a' + tab(3) = 4
}

#[test]
fn test_display_line_count_unicode_zero_width() {
    // Terminal width 0 always returns 1
    assert_eq!(display_line_count_unicode("\u{4E2D}\u{6587}", 0, 4), 1);
}

#[test]
fn test_display_line_count_unicode_empty() {
    // Empty line returns 1
    assert_eq!(display_line_count_unicode("", 80, 4), 1);
}

#[test]
fn test_display_position_unicode_zero_width() {
    // Terminal width 0: returns (0, display_col)
    let line = "a\u{4E2D}b";
    let (dl, dc) = display_position_unicode(line, 2, 0, 4);
    assert_eq!(dl, 0);
    assert_eq!(dc, 3); // 'a'(1) + CJK(2) = 3
}

#[test]
fn test_display_width_unicode_empty() {
    assert_eq!(display_width_unicode("", 4), 0);
}

#[test]
fn test_buffer_col_from_display_col_past_end() {
    // Target display col beyond line length
    let line = "abc";
    assert_eq!(buffer_col_from_display_col(line, 100, 4), 3);
}

#[test]
fn test_buffer_col_from_display_col_unicode_past_end() {
    // Target display col beyond line length
    let line = "a\u{4E2D}";
    assert_eq!(buffer_col_from_display_col_unicode(line, 100, 4), 2);
}

#[test]
fn test_display_col_from_buffer_col_past_end() {
    // Buffer col beyond line length
    let line = "abc";
    assert_eq!(display_col_from_buffer_col(line, 100, 4), 3);
}

#[test]
fn test_display_col_from_buffer_col_unicode_past_end() {
    // Buffer col beyond line length
    let line = "a\u{4E2D}";
    assert_eq!(display_col_from_buffer_col_unicode(line, 100, 4), 3);
}

// =========================================================================
// #717 repro: display_line_count uses chars().count() not Unicode width.
// CJK chars occupy 2 terminal columns each but are counted as 1.
// display_lines.rs:60-71
// =========================================================================

#[test]
fn b6_repro_cjk_chars_miscounted() {
    // 40 CJK characters: each occupies 2 terminal columns = 80 display columns.
    // On a 40-column terminal, this should wrap to 2 display lines.
    let cjk_40 = "\u{4E2D}".repeat(40); // 40 x '中'
    assert_eq!(cjk_40.chars().count(), 40);

    // BUG: display_line_count counts 40 chars / 40 cols = 1 line
    // CORRECT: 80 display columns / 40 cols = 2 lines
    assert_eq!(display_line_count(&cjk_40, 40), 1, "#717: says 1 line (should be 2)");
}

#[test]
fn b6_repro_mixed_ascii_cjk() {
    // "Hello中文World" = 5 + 2*2 + 5 = 14 display columns, 12 chars
    let mixed = "Hello\u{4E2D}\u{6587}World";
    assert_eq!(mixed.chars().count(), 12);

    // On 12-col terminal:
    // BUG: 12 chars / 12 cols = 1 line
    // CORRECT: 14 display cols / 12 cols = 2 lines
    assert_eq!(display_line_count(mixed, 12), 1, "#717: says 1 line (should be 2)");
}
