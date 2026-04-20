use {
    super::*,
    crate::{Cursor, Direction, LinePosition, Position, SimpleText, WordBoundary},
};

fn make_buffer(content: &str) -> SimpleText {
    SimpleText::new(content)
}

fn make_cursor(line: usize, column: usize) -> Cursor {
    Cursor::new(Position::new(line, column))
}

#[test]
fn test_char_motion_forward() {
    let buffer = make_buffer("hello");
    let cursor = make_cursor(0, 0);

    let pos = MotionEngine::calculate(&buffer, &cursor, Motion::Char(Direction::Forward), 1);
    assert_eq!(pos, Some(Position::new(0, 1)));

    // With count
    let pos = MotionEngine::calculate(&buffer, &cursor, Motion::Char(Direction::Forward), 3);
    assert_eq!(pos, Some(Position::new(0, 3)));

    // Clamped to line end
    let pos = MotionEngine::calculate(&buffer, &cursor, Motion::Char(Direction::Forward), 10);
    assert_eq!(pos, Some(Position::new(0, 4))); // "hello" has 5 chars, max column is 4
}

#[test]
fn test_char_motion_backward() {
    let buffer = make_buffer("hello");
    let cursor = make_cursor(0, 4);

    let pos = MotionEngine::calculate(&buffer, &cursor, Motion::Char(Direction::Backward), 1);
    assert_eq!(pos, Some(Position::new(0, 3)));

    // Clamped to 0
    let pos = MotionEngine::calculate(&buffer, &cursor, Motion::Char(Direction::Backward), 10);
    assert_eq!(pos, Some(Position::new(0, 0)));
}

#[test]
fn test_line_motion() {
    let buffer = make_buffer("line1\nline2\nline3");
    let cursor = make_cursor(0, 0);

    let pos = MotionEngine::calculate(&buffer, &cursor, Motion::Line(Direction::Forward), 1);
    assert_eq!(pos, Some(Position::new(1, 0)));

    let pos = MotionEngine::calculate(&buffer, &cursor, Motion::Line(Direction::Forward), 2);
    assert_eq!(pos, Some(Position::new(2, 0)));

    // Clamped to last line
    let pos = MotionEngine::calculate(&buffer, &cursor, Motion::Line(Direction::Forward), 10);
    assert_eq!(pos, Some(Position::new(2, 0)));
}

#[test]
fn test_line_position() {
    let buffer = make_buffer("  hello world  ");
    let cursor = make_cursor(0, 5);

    let pos =
        MotionEngine::calculate(&buffer, &cursor, Motion::LinePosition(LinePosition::Start), 1);
    assert_eq!(pos, Some(Position::new(0, 0)));

    let pos = MotionEngine::calculate(
        &buffer,
        &cursor,
        Motion::LinePosition(LinePosition::FirstNonBlank),
        1,
    );
    assert_eq!(pos, Some(Position::new(0, 2))); // First 'h'

    let pos = MotionEngine::calculate(&buffer, &cursor, Motion::LinePosition(LinePosition::End), 1);
    assert_eq!(pos, Some(Position::new(0, 14))); // Last char index

    let pos = MotionEngine::calculate(
        &buffer,
        &cursor,
        Motion::LinePosition(LinePosition::LastNonBlank),
        1,
    );
    assert_eq!(pos, Some(Position::new(0, 12))); // Last 'd'
}

#[test]
fn test_word_forward() {
    let buffer = make_buffer("hello world foo");
    let cursor = make_cursor(0, 0);

    let pos = MotionEngine::calculate(
        &buffer,
        &cursor,
        Motion::Word {
            direction: Direction::Forward,
            boundary: WordBoundary::Word,
            end: false,
        },
        1,
    );
    assert_eq!(pos, Some(Position::new(0, 6))); // 'w' of world
}

#[test]
fn test_word_backward() {
    let buffer = make_buffer("hello world foo");
    let cursor = make_cursor(0, 12);

    let pos = MotionEngine::calculate(
        &buffer,
        &cursor,
        Motion::Word {
            direction: Direction::Backward,
            boundary: WordBoundary::Word,
            end: false,
        },
        1,
    );
    assert_eq!(pos, Some(Position::new(0, 6))); // 'w' of world
}

#[test]
fn test_word_end() {
    let buffer = make_buffer("hello world foo");
    let cursor = make_cursor(0, 0);

    let pos = MotionEngine::calculate(
        &buffer,
        &cursor,
        Motion::Word {
            direction: Direction::Forward,
            boundary: WordBoundary::Word,
            end: true,
        },
        1,
    );
    assert_eq!(pos, Some(Position::new(0, 4))); // 'o' of hello
}

#[test]
fn test_jump_line() {
    let buffer = make_buffer("line0\nline1\nline2");
    let cursor = make_cursor(0, 0);

    // G (no count) goes to last line
    let pos = MotionEngine::calculate(&buffer, &cursor, Motion::JumpLine(None), 1);
    assert_eq!(pos, Some(Position::new(2, 0)));

    // Jump to specific line
    let pos = MotionEngine::calculate(&buffer, &cursor, Motion::JumpLine(Some(1)), 1);
    assert_eq!(pos, Some(Position::new(1, 0)));
}

#[test]
fn test_match_bracket() {
    let buffer = make_buffer("(hello)");
    let cursor = make_cursor(0, 0);

    let pos = MotionEngine::calculate(&buffer, &cursor, Motion::MatchBracket, 1);
    assert_eq!(pos, Some(Position::new(0, 6))); // closing )

    // From closing bracket
    let cursor = make_cursor(0, 6);
    let pos = MotionEngine::calculate(&buffer, &cursor, Motion::MatchBracket, 1);
    assert_eq!(pos, Some(Position::new(0, 0))); // opening (
}

#[test]
fn test_match_bracket_nested() {
    let buffer = make_buffer("((inner))");
    let cursor = make_cursor(0, 0);

    let pos = MotionEngine::calculate(&buffer, &cursor, Motion::MatchBracket, 1);
    assert_eq!(pos, Some(Position::new(0, 8))); // outermost closing )
}

#[test]
fn test_find_char() {
    let buffer = make_buffer("hello world");
    let cursor = make_cursor(0, 0);

    // f (find forward)
    let pos = MotionEngine::calculate(
        &buffer,
        &cursor,
        Motion::FindChar {
            char: 'w',
            direction: Direction::Forward,
            till: false,
        },
        1,
    );
    assert_eq!(pos, Some(Position::new(0, 6)));

    // t (till forward)
    let pos = MotionEngine::calculate(
        &buffer,
        &cursor,
        Motion::FindChar {
            char: 'w',
            direction: Direction::Forward,
            till: true,
        },
        1,
    );
    assert_eq!(pos, Some(Position::new(0, 5)));
}

#[test]
fn test_paragraph_motion() {
    let buffer = make_buffer("para1\n\npara2\npara2b\n\npara3");
    let cursor = make_cursor(0, 0);

    // } (forward)
    let pos = MotionEngine::calculate(&buffer, &cursor, Motion::Paragraph(Direction::Forward), 1);
    assert_eq!(pos.map(|p| p.line), Some(2)); // Start of para2

    // { (backward) from para3
    let cursor = make_cursor(5, 0);
    let pos = MotionEngine::calculate(&buffer, &cursor, Motion::Paragraph(Direction::Backward), 1);
    assert_eq!(pos.map(|p| p.line), Some(2)); // Start of para2
}

// === Coverage: calculate_with_desired_col ===

#[test]
fn test_calculate_with_desired_col_vertical() {
    let buffer = make_buffer("hello\nhi\nworld");
    let cursor = make_cursor(0, 4);

    // Vertical motion should preserve desired column
    let (new_pos, desired_col) = MotionEngine::calculate_with_desired_col(
        &buffer,
        &cursor,
        Motion::Line(Direction::Forward),
        1,
    );
    assert_eq!(new_pos, Some(Position::new(1, 1))); // "hi" only 2 chars, clamped
    assert_eq!(desired_col, Some(4)); // Desired column preserved
}

#[test]
fn test_calculate_with_desired_col_horizontal() {
    let buffer = make_buffer("hello");
    let cursor = make_cursor(0, 0);

    // Horizontal motion should clear desired column
    let (new_pos, desired_col) = MotionEngine::calculate_with_desired_col(
        &buffer,
        &cursor,
        Motion::Char(Direction::Forward),
        1,
    );
    assert_eq!(new_pos, Some(Position::new(0, 1)));
    assert_eq!(desired_col, None); // Cleared for horizontal
}

#[test]
fn test_calculate_with_desired_col_with_preferred() {
    let buffer = make_buffer("hello\nhi\nworld");
    let mut cursor = make_cursor(0, 4);
    cursor.preferred_column = Some(4);

    let (new_pos, desired_col) = MotionEngine::calculate_with_desired_col(
        &buffer,
        &cursor,
        Motion::Line(Direction::Forward),
        1,
    );
    assert_eq!(new_pos, Some(Position::new(1, 1)));
    assert_eq!(desired_col, Some(4));
}

// === Coverage: BigWord motions ===

#[test]
fn test_big_word_forward() {
    let buffer = make_buffer("hello.world foo");
    let cursor = make_cursor(0, 0);

    let pos = MotionEngine::calculate(
        &buffer,
        &cursor,
        Motion::Word {
            direction: Direction::Forward,
            boundary: WordBoundary::BigWord,
            end: false,
        },
        1,
    );
    assert_eq!(pos, Some(Position::new(0, 12))); // 'f' of foo (skip hello.world as one WORD)
}

#[test]
fn test_big_word_end() {
    let buffer = make_buffer("hello.world foo");
    let cursor = make_cursor(0, 0);

    let pos = MotionEngine::calculate(
        &buffer,
        &cursor,
        Motion::Word {
            direction: Direction::Forward,
            boundary: WordBoundary::BigWord,
            end: true,
        },
        1,
    );
    assert_eq!(pos, Some(Position::new(0, 10))); // 'd' of hello.world
}

#[test]
fn test_big_word_backward() {
    let buffer = make_buffer("hello.world foo");
    let cursor = make_cursor(0, 14);

    let pos = MotionEngine::calculate(
        &buffer,
        &cursor,
        Motion::Word {
            direction: Direction::Backward,
            boundary: WordBoundary::BigWord,
            end: false,
        },
        1,
    );
    assert_eq!(pos, Some(Position::new(0, 12))); // 'f' of foo
}

// === Coverage: word_end_backward (ge motion) ===

#[test]
fn test_word_end_backward() {
    let buffer = make_buffer("hello world foo");
    let cursor = make_cursor(0, 12);

    let pos = MotionEngine::calculate(
        &buffer,
        &cursor,
        Motion::Word {
            direction: Direction::Backward,
            boundary: WordBoundary::Word,
            end: true,
        },
        1,
    );
    assert_eq!(pos, Some(Position::new(0, 10))); // 'd' of world
}

#[test]
fn test_word_end_backward_at_start() {
    let buffer = make_buffer("hello world");
    let cursor = make_cursor(0, 0);

    let pos = MotionEngine::calculate(
        &buffer,
        &cursor,
        Motion::Word {
            direction: Direction::Backward,
            boundary: WordBoundary::Word,
            end: true,
        },
        1,
    );
    // At start, backs up to (empty buffer check passes, then backs up 1)
    assert!(pos.is_some());
}

#[test]
fn test_word_end_backward_empty_buffer() {
    let buffer = SimpleText::new("");
    let cursor = make_cursor(0, 0);

    let pos = MotionEngine::calculate(
        &buffer,
        &cursor,
        Motion::Word {
            direction: Direction::Backward,
            boundary: WordBoundary::Word,
            end: true,
        },
        1,
    );
    assert_eq!(pos, Some(Position::new(0, 0)));
}

#[test]
fn test_word_end_backward_multiline() {
    let buffer = make_buffer("hello\n\nworld");
    let cursor = make_cursor(2, 0);

    let pos = MotionEngine::calculate(
        &buffer,
        &cursor,
        Motion::Word {
            direction: Direction::Backward,
            boundary: WordBoundary::Word,
            end: true,
        },
        1,
    );
    assert_eq!(pos, Some(Position::new(0, 4))); // 'o' of hello
}

// === Coverage: backward paragraph with multiple counts ===

#[test]
fn test_paragraph_backward_multiple_count() {
    let buffer = make_buffer("para1\n\npara2\n\npara3");
    let cursor = make_cursor(4, 0);

    let pos = MotionEngine::calculate(&buffer, &cursor, Motion::Paragraph(Direction::Backward), 2);
    assert_eq!(pos.map(|p| p.line), Some(0)); // Back to para1
}

#[test]
fn test_paragraph_forward_multiple_count() {
    let buffer = make_buffer("para1\n\npara2\n\npara3");
    let cursor = make_cursor(0, 0);

    let pos = MotionEngine::calculate(&buffer, &cursor, Motion::Paragraph(Direction::Forward), 2);
    assert_eq!(pos.map(|p| p.line), Some(4)); // Start of para3
}

#[test]
fn test_paragraph_empty_buffer() {
    let buffer = SimpleText::new("");
    let cursor = make_cursor(0, 0);

    let pos = MotionEngine::calculate(&buffer, &cursor, Motion::Paragraph(Direction::Forward), 1);
    assert_eq!(pos, None);
}

// === Coverage: find_char backward and till backward ===

#[test]
fn test_find_char_backward_hello() {
    let buffer = make_buffer("hello world");
    let cursor = make_cursor(0, 10);

    let pos = MotionEngine::calculate(
        &buffer,
        &cursor,
        Motion::FindChar {
            char: 'o',
            direction: Direction::Backward,
            till: false,
        },
        1,
    );
    assert_eq!(pos, Some(Position::new(0, 7))); // 'o' in world
}

#[test]
fn test_find_char_till_backward() {
    let buffer = make_buffer("hello world");
    let cursor = make_cursor(0, 10);

    let pos = MotionEngine::calculate(
        &buffer,
        &cursor,
        Motion::FindChar {
            char: 'o',
            direction: Direction::Backward,
            till: true,
        },
        1,
    );
    assert_eq!(pos, Some(Position::new(0, 8))); // one after 'o'
}

#[test]
fn test_find_char_not_found() {
    let buffer = make_buffer("hello");
    let cursor = make_cursor(0, 0);

    let pos = MotionEngine::calculate(
        &buffer,
        &cursor,
        Motion::FindChar {
            char: 'z',
            direction: Direction::Forward,
            till: false,
        },
        1,
    );
    assert_eq!(pos, None);
}

#[test]
fn test_find_char_with_count() {
    let buffer = make_buffer("aabaa");
    let cursor = make_cursor(0, 0);

    let pos = MotionEngine::calculate(
        &buffer,
        &cursor,
        Motion::FindChar {
            char: 'a',
            direction: Direction::Forward,
            till: false,
        },
        2,
    );
    assert_eq!(pos, Some(Position::new(0, 3))); // second 'a' after cursor
}

// === Coverage: match_bracket search forward on line ===

#[test]
fn test_match_bracket_search_forward() {
    let buffer = make_buffer("hello (world)");
    let cursor = make_cursor(0, 0); // Not on a bracket

    let pos = MotionEngine::calculate(&buffer, &cursor, Motion::MatchBracket, 1);
    assert_eq!(pos, Some(Position::new(0, 12))); // Matching )
}

#[test]
fn test_match_bracket_multiline() {
    let buffer = make_buffer("(\n  content\n)");
    let cursor = make_cursor(0, 0);

    let pos = MotionEngine::calculate(&buffer, &cursor, Motion::MatchBracket, 1);
    assert_eq!(pos, Some(Position::new(2, 0)));
}

#[test]
fn test_match_bracket_multiline_backward() {
    let buffer = make_buffer("(\n  content\n)");
    let cursor = make_cursor(2, 0);

    let pos = MotionEngine::calculate(&buffer, &cursor, Motion::MatchBracket, 1);
    assert_eq!(pos, Some(Position::new(0, 0)));
}

#[test]
fn test_match_bracket_square() {
    let buffer = make_buffer("[hello]");
    let cursor = make_cursor(0, 0);

    let pos = MotionEngine::calculate(&buffer, &cursor, Motion::MatchBracket, 1);
    assert_eq!(pos, Some(Position::new(0, 6)));

    let cursor = make_cursor(0, 6);
    let pos = MotionEngine::calculate(&buffer, &cursor, Motion::MatchBracket, 1);
    assert_eq!(pos, Some(Position::new(0, 0)));
}

#[test]
fn test_match_bracket_curly() {
    let buffer = make_buffer("{hello}");
    let cursor = make_cursor(0, 0);

    let pos = MotionEngine::calculate(&buffer, &cursor, Motion::MatchBracket, 1);
    assert_eq!(pos, Some(Position::new(0, 6)));

    let cursor = make_cursor(0, 6);
    let pos = MotionEngine::calculate(&buffer, &cursor, Motion::MatchBracket, 1);
    assert_eq!(pos, Some(Position::new(0, 0)));
}

#[test]
fn test_match_bracket_no_bracket() {
    let buffer = make_buffer("hello world");
    let cursor = make_cursor(0, 0);

    let pos = MotionEngine::calculate(&buffer, &cursor, Motion::MatchBracket, 1);
    assert_eq!(pos, None);
}

// === Coverage: word_forward punctuation ===

#[test]
fn test_word_forward_punctuation() {
    let buffer = make_buffer("foo::bar baz");
    let cursor = make_cursor(0, 0);

    let pos = MotionEngine::calculate(
        &buffer,
        &cursor,
        Motion::Word {
            direction: Direction::Forward,
            boundary: WordBoundary::Word,
            end: false,
        },
        1,
    );
    assert_eq!(pos, Some(Position::new(0, 3))); // :: is punctuation
}

#[test]
fn test_word_backward_punctuation() {
    let buffer = make_buffer("foo::bar");
    let cursor = make_cursor(0, 7);

    let pos = MotionEngine::calculate(
        &buffer,
        &cursor,
        Motion::Word {
            direction: Direction::Backward,
            boundary: WordBoundary::Word,
            end: false,
        },
        1,
    );
    assert_eq!(pos, Some(Position::new(0, 5))); // 'b' of bar
}

// === Coverage: word_end punctuation ===

#[test]
fn test_word_end_punctuation() {
    let buffer = make_buffer("foo::bar");
    let cursor = make_cursor(0, 0);

    let pos = MotionEngine::calculate(
        &buffer,
        &cursor,
        Motion::Word {
            direction: Direction::Forward,
            boundary: WordBoundary::Word,
            end: true,
        },
        1,
    );
    assert_eq!(pos, Some(Position::new(0, 2))); // last 'o' of foo
}

// === Coverage: line_motion edge cases ===

#[test]
fn test_line_motion_backward() {
    let buffer = make_buffer("line1\nline2\nline3");
    let cursor = make_cursor(2, 0);

    let pos = MotionEngine::calculate(&buffer, &cursor, Motion::Line(Direction::Backward), 1);
    assert_eq!(pos, Some(Position::new(1, 0)));

    // Clamped to first line
    let pos = MotionEngine::calculate(&buffer, &cursor, Motion::Line(Direction::Backward), 10);
    assert_eq!(pos, Some(Position::new(0, 0)));
}

#[test]
fn test_line_motion_empty_buffer() {
    let buffer = SimpleText::new("");
    let cursor = make_cursor(0, 0);

    let pos = MotionEngine::calculate(&buffer, &cursor, Motion::Line(Direction::Forward), 1);
    assert_eq!(pos, None);
}

#[test]
fn test_line_motion_empty_target_line() {
    let buffer = make_buffer("hello\n\nworld");
    let cursor = make_cursor(0, 3);

    let pos = MotionEngine::calculate(&buffer, &cursor, Motion::Line(Direction::Forward), 1);
    assert_eq!(pos, Some(Position::new(1, 0))); // empty line, col clamped to 0
}

// === Coverage: jump_line edge cases ===

#[test]
fn test_jump_line_with_indented_target() {
    let buffer = make_buffer("hello\n  world");
    let cursor = make_cursor(0, 0);

    let pos = MotionEngine::calculate(&buffer, &cursor, Motion::JumpLine(Some(1)), 1);
    assert_eq!(pos, Some(Position::new(1, 2))); // first non-blank is 'w'
}

#[test]
fn test_jump_line_beyond_end() {
    let buffer = make_buffer("line1\nline2");
    let cursor = make_cursor(0, 0);

    let pos = MotionEngine::calculate(&buffer, &cursor, Motion::JumpLine(Some(100)), 1);
    assert_eq!(pos, Some(Position::new(1, 0))); // clamped to last line
}

#[test]
fn test_jump_line_empty_buffer() {
    let buffer = SimpleText::new("");
    let cursor = make_cursor(0, 0);

    let pos = MotionEngine::calculate(&buffer, &cursor, Motion::JumpLine(None), 1);
    assert_eq!(pos, None);
}

// === Coverage: word_forward multiline ===

#[test]
fn test_word_forward_multiline() {
    let buffer = make_buffer("hello\nworld");
    let cursor = make_cursor(0, 4);

    let pos = MotionEngine::calculate(
        &buffer,
        &cursor,
        Motion::Word {
            direction: Direction::Forward,
            boundary: WordBoundary::Word,
            end: false,
        },
        1,
    );
    assert_eq!(pos, Some(Position::new(1, 0))); // 'w' of world
}

#[test]
fn test_word_backward_multiline() {
    let buffer = make_buffer("hello\nworld");
    let cursor = make_cursor(1, 0);

    let pos = MotionEngine::calculate(
        &buffer,
        &cursor,
        Motion::Word {
            direction: Direction::Backward,
            boundary: WordBoundary::Word,
            end: false,
        },
        1,
    );
    assert_eq!(pos, Some(Position::new(0, 0))); // 'h' of hello
}

// === Coverage: word_forward starting on whitespace ===

#[test]
fn test_word_forward_from_whitespace() {
    let buffer = make_buffer("  hello");
    let cursor = make_cursor(0, 0);

    let pos = MotionEngine::calculate(
        &buffer,
        &cursor,
        Motion::Word {
            direction: Direction::Forward,
            boundary: WordBoundary::Word,
            end: false,
        },
        1,
    );
    assert_eq!(pos, Some(Position::new(0, 2))); // 'h' of hello
}

// === Coverage: count 0 treated as 1 ===

#[test]
fn test_count_zero_treated_as_one() {
    let buffer = make_buffer("hello");
    let cursor = make_cursor(0, 0);

    let pos = MotionEngine::calculate(&buffer, &cursor, Motion::Char(Direction::Forward), 0);
    assert_eq!(pos, Some(Position::new(0, 1))); // count 0 -> 1
}

// === Coverage: line_position edge cases ===

#[test]
fn test_line_position_all_whitespace() {
    let buffer = make_buffer("   ");
    let cursor = make_cursor(0, 1);

    let pos = MotionEngine::calculate(
        &buffer,
        &cursor,
        Motion::LinePosition(LinePosition::FirstNonBlank),
        1,
    );
    assert_eq!(pos, Some(Position::new(0, 0))); // no non-blank, defaults to 0

    let pos = MotionEngine::calculate(
        &buffer,
        &cursor,
        Motion::LinePosition(LinePosition::LastNonBlank),
        1,
    );
    assert_eq!(pos, Some(Position::new(0, 0)));
}

// === Coverage: word_forward BigWord on whitespace start ===

#[test]
fn test_word_forward_bigword_from_whitespace() {
    let buffer = make_buffer("  hello.world");
    let cursor = make_cursor(0, 0);

    let pos = MotionEngine::calculate(
        &buffer,
        &cursor,
        Motion::Word {
            direction: Direction::Forward,
            boundary: WordBoundary::BigWord,
            end: false,
        },
        1,
    );
    assert_eq!(pos, Some(Position::new(0, 2)));
}

// === Coverage: word_forward punctuation skip ===

#[test]
fn test_word_forward_punctuation_skip() {
    let buffer = make_buffer("foo::bar baz");
    let cursor = make_cursor(0, 3); // on first ':'

    let pos = MotionEngine::calculate(
        &buffer,
        &cursor,
        Motion::Word {
            direction: Direction::Forward,
            boundary: WordBoundary::Word,
            end: false,
        },
        1,
    );
    // Skip '::' punctuation, then 'bar', then ' ', land on 'baz'
    assert_eq!(pos, Some(Position::new(0, 5)));
}

// === Coverage: word_forward cross-line with whitespace-only lines ===

#[test]
fn test_word_forward_across_whitespace_only_line() {
    let buffer = make_buffer("hello\n   \nworld");
    let cursor = make_cursor(0, 4);

    let pos = MotionEngine::calculate(
        &buffer,
        &cursor,
        Motion::Word {
            direction: Direction::Forward,
            boundary: WordBoundary::Word,
            end: false,
        },
        1,
    );
    // Should skip whitespace-only line and land on 'world'
    assert_eq!(pos, Some(Position::new(2, 0)));
}

// === Coverage: word_forward at very end of buffer ===

#[test]
fn test_word_forward_at_end_of_buffer() {
    let buffer = make_buffer("hello");
    let cursor = make_cursor(0, 4);

    let pos = MotionEngine::calculate(
        &buffer,
        &cursor,
        Motion::Word {
            direction: Direction::Forward,
            boundary: WordBoundary::Word,
            end: false,
        },
        1,
    );
    assert_eq!(pos, Some(Position::new(0, 4)));
}

// === Coverage: word_end BigWord ===

// === Coverage: word_end cross-line ===

#[test]
fn test_word_end_cross_line() {
    let buffer = make_buffer("hello\nworld");
    let cursor = make_cursor(0, 4); // at end of 'hello'

    let pos = MotionEngine::calculate(
        &buffer,
        &cursor,
        Motion::Word {
            direction: Direction::Forward,
            boundary: WordBoundary::Word,
            end: true,
        },
        1,
    );
    assert_eq!(pos, Some(Position::new(1, 4)));
}

#[test]
fn test_word_end_empty_buffer() {
    let buffer = SimpleText::new("");
    let cursor = make_cursor(0, 0);

    let pos = MotionEngine::calculate(
        &buffer,
        &cursor,
        Motion::Word {
            direction: Direction::Forward,
            boundary: WordBoundary::Word,
            end: true,
        },
        1,
    );
    assert_eq!(pos, Some(Position::new(0, 0)));
}

#[test]
fn test_word_end_across_whitespace_only_line() {
    let buffer = make_buffer("hello\n   \nworld");
    let cursor = make_cursor(0, 4);

    let pos = MotionEngine::calculate(
        &buffer,
        &cursor,
        Motion::Word {
            direction: Direction::Forward,
            boundary: WordBoundary::Word,
            end: true,
        },
        1,
    );
    assert_eq!(pos, Some(Position::new(2, 4)));
}

// === Coverage: word_end punctuation in Word mode ===

#[test]
fn test_word_end_on_punctuation() {
    let buffer = make_buffer("foo::bar");
    let cursor = make_cursor(0, 2); // 'o' of foo, next should be '::'

    let pos = MotionEngine::calculate(
        &buffer,
        &cursor,
        Motion::Word {
            direction: Direction::Forward,
            boundary: WordBoundary::Word,
            end: true,
        },
        1,
    );
    assert_eq!(pos, Some(Position::new(0, 4))); // end of '::'
}

// === Coverage: word_backward empty buffer ===

#[test]
fn test_word_backward_empty_buffer() {
    let buffer = SimpleText::new("");
    let cursor = make_cursor(0, 0);

    let pos = MotionEngine::calculate(
        &buffer,
        &cursor,
        Motion::Word {
            direction: Direction::Backward,
            boundary: WordBoundary::Word,
            end: false,
        },
        1,
    );
    assert_eq!(pos, Some(Position::new(0, 0)));
}

// === Coverage: word_backward cross-line with whitespace-only first line ===

#[test]
fn test_word_backward_across_whitespace_line() {
    let buffer = make_buffer("hello\n   \nworld");
    let cursor = make_cursor(2, 0);

    let pos = MotionEngine::calculate(
        &buffer,
        &cursor,
        Motion::Word {
            direction: Direction::Backward,
            boundary: WordBoundary::Word,
            end: false,
        },
        1,
    );
    assert_eq!(pos, Some(Position::new(0, 0)));
}

// === Coverage: word_backward BigWord ===

#[test]
fn test_word_backward_bigword() {
    let buffer = make_buffer("hello foo.bar baz");
    let cursor = make_cursor(0, 14);

    let pos = MotionEngine::calculate(
        &buffer,
        &cursor,
        Motion::Word {
            direction: Direction::Backward,
            boundary: WordBoundary::BigWord,
            end: false,
        },
        1,
    );
    assert_eq!(pos, Some(Position::new(0, 6)));
}

// === Coverage: word_backward punctuation ===

#[test]
fn test_word_backward_from_punct() {
    let buffer = make_buffer("foo::bar");
    let cursor = make_cursor(0, 4);

    let pos = MotionEngine::calculate(
        &buffer,
        &cursor,
        Motion::Word {
            direction: Direction::Backward,
            boundary: WordBoundary::Word,
            end: false,
        },
        1,
    );
    assert_eq!(pos, Some(Position::new(0, 3)));
}

// === Coverage: word_end_backward ===

#[test]
fn test_word_end_backward_basic() {
    let buffer = make_buffer("hello world");
    let cursor = make_cursor(0, 10);

    let pos = MotionEngine::calculate(
        &buffer,
        &cursor,
        Motion::Word {
            direction: Direction::Backward,
            boundary: WordBoundary::Word,
            end: true,
        },
        1,
    );
    // ge from 'd' (col 10) moves backward
    assert!(pos.is_some());
}

#[test]
fn test_word_end_backward_cross_line() {
    let buffer = make_buffer("hello\nworld");
    let cursor = make_cursor(1, 0);

    let pos = MotionEngine::calculate(
        &buffer,
        &cursor,
        Motion::Word {
            direction: Direction::Backward,
            boundary: WordBoundary::Word,
            end: true,
        },
        1,
    );
    assert_eq!(pos, Some(Position::new(0, 4)));
}

#[test]
fn test_word_end_backward_whitespace_only_line() {
    let buffer = make_buffer("hello\n   \nworld");
    let cursor = make_cursor(2, 0);

    let pos = MotionEngine::calculate(
        &buffer,
        &cursor,
        Motion::Word {
            direction: Direction::Backward,
            boundary: WordBoundary::Word,
            end: true,
        },
        1,
    );
    assert_eq!(pos, Some(Position::new(0, 4)));
}

// === Coverage: paragraph backward with count ===

#[test]
fn test_paragraph_backward_count2() {
    let buffer = make_buffer("para1\n\npara2\n\npara3");
    let cursor = make_cursor(4, 0);

    let pos = MotionEngine::calculate(&buffer, &cursor, Motion::Paragraph(Direction::Backward), 2);
    assert!(pos.is_some());
    assert_eq!(pos.unwrap().line, 0);
}

#[test]
fn test_paragraph_backward_from_empty_line() {
    let buffer = make_buffer("para1\n\npara2");
    let cursor = make_cursor(1, 0); // on empty line

    let pos = MotionEngine::calculate(&buffer, &cursor, Motion::Paragraph(Direction::Backward), 1);
    assert!(pos.is_some());
    assert_eq!(pos.unwrap().line, 0);
}

#[test]
fn test_paragraph_forward_count2() {
    let buffer = make_buffer("para1\n\npara2\n\npara3");
    let cursor = make_cursor(0, 0);

    let pos = MotionEngine::calculate(&buffer, &cursor, Motion::Paragraph(Direction::Forward), 2);
    assert!(pos.is_some());
    assert_eq!(pos.unwrap().line, 4);
}

// === Coverage: find_char backward ===

#[test]
fn test_find_char_backward() {
    let buffer = make_buffer("abcabc");
    let cursor = make_cursor(0, 5);

    let pos = MotionEngine::calculate(
        &buffer,
        &cursor,
        Motion::FindChar {
            char: 'a',
            direction: Direction::Backward,
            till: false,
        },
        1,
    );
    assert_eq!(pos, Some(Position::new(0, 3)));
}

#[test]
fn test_find_char_backward_till() {
    let buffer = make_buffer("abcabc");
    let cursor = make_cursor(0, 5);

    let pos = MotionEngine::calculate(
        &buffer,
        &cursor,
        Motion::FindChar {
            char: 'a',
            direction: Direction::Backward,
            till: true,
        },
        1,
    );
    assert_eq!(pos, Some(Position::new(0, 4)));
}

#[test]
fn test_find_char_backward_count2() {
    let buffer = make_buffer("abcabc");
    let cursor = make_cursor(0, 5);

    let pos = MotionEngine::calculate(
        &buffer,
        &cursor,
        Motion::FindChar {
            char: 'a',
            direction: Direction::Backward,
            till: false,
        },
        2,
    );
    assert_eq!(pos, Some(Position::new(0, 0)));
}

// === Coverage: match_bracket forward search on same line ===

#[test]
fn test_match_bracket_closing_search_forward() {
    let buffer = make_buffer("x ) ( y");
    let cursor = make_cursor(0, 0);

    let pos = MotionEngine::calculate(&buffer, &cursor, Motion::MatchBracket, 1);
    // Forward search finds ')' at col 2 but no matching '(' before it
    assert_eq!(pos, None);
}

#[test]
fn test_match_bracket_multiline_forward() {
    let buffer = make_buffer("(\nhello\n)");
    let cursor = make_cursor(0, 0);

    let pos = MotionEngine::calculate(&buffer, &cursor, Motion::MatchBracket, 1);
    assert_eq!(pos, Some(Position::new(2, 0)));
}

#[test]
fn test_match_bracket_multiline_backward_newline() {
    let buffer = make_buffer("(\nhello\n)");
    let cursor = make_cursor(2, 0);

    let pos = MotionEngine::calculate(&buffer, &cursor, Motion::MatchBracket, 1);
    assert_eq!(pos, Some(Position::new(0, 0)));
}

#[test]
fn test_match_bracket_square_simple() {
    let buffer = make_buffer("[hello]");
    let cursor = make_cursor(0, 0);

    let pos = MotionEngine::calculate(&buffer, &cursor, Motion::MatchBracket, 1);
    assert_eq!(pos, Some(Position::new(0, 6)));
}

#[test]
fn test_match_bracket_curly_simple() {
    let buffer = make_buffer("{hello}");
    let cursor = make_cursor(0, 0);

    let pos = MotionEngine::calculate(&buffer, &cursor, Motion::MatchBracket, 1);
    assert_eq!(pos, Some(Position::new(0, 6)));
}

// === Coverage: calculate_with_desired_col ===

#[test]
fn test_calculate_with_desired_col_line_motion() {
    let buffer = make_buffer("hello\nworld");
    let cursor = make_cursor(0, 3);

    let (pos, desired) = MotionEngine::calculate_with_desired_col(
        &buffer,
        &cursor,
        Motion::Line(Direction::Forward),
        1,
    );
    assert_eq!(pos, Some(Position::new(1, 3)));
    assert_eq!(desired, Some(3));
}

#[test]
fn test_calculate_with_desired_col_char_motion() {
    let buffer = make_buffer("hello");
    let cursor = make_cursor(0, 0);

    let (pos, desired) = MotionEngine::calculate_with_desired_col(
        &buffer,
        &cursor,
        Motion::Char(Direction::Forward),
        1,
    );
    assert_eq!(pos, Some(Position::new(0, 1)));
    assert_eq!(desired, None); // Horizontal motion clears desired col
}

// === Coverage: word_forward empty buffer ===

#[test]
fn test_word_forward_empty_buffer() {
    let buffer = SimpleText::new("");
    let cursor = make_cursor(0, 0);

    let pos = MotionEngine::calculate(
        &buffer,
        &cursor,
        Motion::Word {
            direction: Direction::Forward,
            boundary: WordBoundary::Word,
            end: false,
        },
        1,
    );
    assert_eq!(pos, Some(Position::new(0, 0)));
}

// === Coverage: word_end at last line last col ===

#[test]
fn test_word_end_at_last_position() {
    let buffer = make_buffer("hello");
    let cursor = make_cursor(0, 4);

    let pos = MotionEngine::calculate(
        &buffer,
        &cursor,
        Motion::Word {
            direction: Direction::Forward,
            boundary: WordBoundary::Word,
            end: true,
        },
        1,
    );
    assert_eq!(pos, Some(Position::new(0, 4)));
}

// === Coverage: paragraph backward to line 0 with empty line at start ===

#[test]
fn test_paragraph_backward_starting_empty() {
    let buffer = make_buffer("\npara1\n\npara2");
    let cursor = make_cursor(3, 0);

    let pos = MotionEngine::calculate(&buffer, &cursor, Motion::Paragraph(Direction::Backward), 1);
    assert!(pos.is_some());
    assert!(pos.unwrap().line <= 1);
}

// === Coverage: backward bracket search spanning multiple lines ===

#[test]
fn test_backward_bracket_multiline_nested() {
    let buffer = make_buffer("(\n  inner\n  (\n    deep\n  )\n)");
    let cursor = make_cursor(5, 0); // on the closing ')'

    let pos = MotionEngine::calculate(&buffer, &cursor, Motion::MatchBracket, 1);
    assert_eq!(pos, Some(Position::new(0, 0)));
}

// === Coverage: word_forward pos.line >= line_count branch ===

// === Coverage: word_end_backward whitespace at start of line ===

#[test]
fn test_word_end_backward_whitespace_at_line_start() {
    let buffer = make_buffer("  hello\nworld");
    let cursor = make_cursor(1, 0);

    let pos = MotionEngine::calculate(
        &buffer,
        &cursor,
        Motion::Word {
            direction: Direction::Backward,
            boundary: WordBoundary::Word,
            end: true,
        },
        1,
    );
    // Should go to 'o' of hello (line 0, col 6)
    assert_eq!(pos, Some(Position::new(0, 6)));
}

// === Coverage: paragraph backward at line 0 with empty first line ===

#[test]
fn test_paragraph_backward_line0_empty() {
    let buffer = make_buffer("\n\npara1");
    let cursor = make_cursor(2, 0);

    let pos = MotionEngine::calculate(&buffer, &cursor, Motion::Paragraph(Direction::Backward), 1);
    assert!(pos.is_some());
    // Paragraph backward from para1 skips empty lines 0,1 -> reaches line 0
    assert_eq!(pos.unwrap().line, 0);
}

// === Coverage: match_bracket forward search finding closing bracket ===

#[test]
fn test_match_bracket_forward_search_finds_close() {
    let buffer = make_buffer("abc ) xyz");
    let cursor = make_cursor(0, 0); // Not on a bracket

    let pos = MotionEngine::calculate(&buffer, &cursor, Motion::MatchBracket, 1);
    // Forward search finds ')' at col 4, which is a closing bracket
    // find_backward_bracket should fail (no matching '('), returns None
    assert!(pos.is_none());
}

// === Coverage: word_end multiline at end of last line ===

#[test]
fn test_word_end_at_end_of_last_line() {
    let buffer = make_buffer("hello world");
    let cursor = make_cursor(0, 10); // at 'd' of "world" (last char)

    let pos = MotionEngine::calculate(
        &buffer,
        &cursor,
        Motion::Word {
            direction: Direction::Forward,
            boundary: WordBoundary::Word,
            end: true,
        },
        1,
    );
    // At last position of last line, stays at end
    assert_eq!(pos, Some(Position::new(0, 10)));
}

// === Coverage: word_backward from whitespace-only start of line ===

#[test]
fn test_word_backward_whitespace_start() {
    let buffer = make_buffer("   hello");
    let cursor = make_cursor(0, 1); // on whitespace

    let pos = MotionEngine::calculate(
        &buffer,
        &cursor,
        Motion::Word {
            direction: Direction::Backward,
            boundary: WordBoundary::Word,
            end: false,
        },
        1,
    );
    // At whitespace start, should go to col 0
    assert_eq!(pos, Some(Position::new(0, 0)));
}

// === Coverage: paragraph forward past end of buffer ===

#[test]
fn test_paragraph_forward_past_end() {
    let buffer = make_buffer("hello");
    let cursor = make_cursor(0, 0);

    let pos = MotionEngine::calculate(&buffer, &cursor, Motion::Paragraph(Direction::Forward), 1);
    // Single line, no empty line, forward should stop at end
    assert!(pos.is_some());
}

// === Coverage: paragraph backward at line 0 non-empty ===

#[test]
fn test_paragraph_backward_at_line0() {
    let buffer = make_buffer("hello\nworld");
    let cursor = make_cursor(0, 0);

    let pos = MotionEngine::calculate(&buffer, &cursor, Motion::Paragraph(Direction::Backward), 1);
    // Already at line 0 with non-empty content, should stay
    assert!(pos.is_some());
    assert_eq!(pos.unwrap().line, 0);
}

// === Coverage: word_end_backward at line 0 col 0 with whitespace ===

#[test]
fn test_word_end_backward_at_origin() {
    let buffer = make_buffer("  hello");
    let cursor = make_cursor(0, 1); // on whitespace

    let pos = MotionEngine::calculate(
        &buffer,
        &cursor,
        Motion::Word {
            direction: Direction::Backward,
            boundary: WordBoundary::Word,
            end: true,
        },
        1,
    );
    // Back up one -> col 0 which is whitespace
    // chars.get(0) is whitespace -> stays on whitespace, col = 0
    assert!(pos.is_some());
    assert_eq!(pos.unwrap().column, 0);
}

// === Coverage: word_forward_empty_line between words ===

#[test]
fn test_word_forward_through_empty_line() {
    let buffer = make_buffer("hello\n\nworld");
    let cursor = make_cursor(0, 4);

    let pos = MotionEngine::calculate(
        &buffer,
        &cursor,
        Motion::Word {
            direction: Direction::Forward,
            boundary: WordBoundary::Word,
            end: false,
        },
        1,
    );
    // Should skip empty line and reach "world"
    assert!(pos.is_some());
}

// === Coverage: calculate_with_desired_col count 0 ===

#[test]
fn test_calculate_with_desired_col_count_zero() {
    let buffer = make_buffer("hello\nworld");
    let cursor = make_cursor(0, 0);

    let (pos, desired) = MotionEngine::calculate_with_desired_col(
        &buffer,
        &cursor,
        Motion::Line(Direction::Forward),
        0,
    );
    // count 0 should be clamped to 1
    assert_eq!(pos, Some(Position::new(1, 0)));
    assert_eq!(desired, Some(0));
}

// === Coverage: word_forward multiline BigWord ===

#[test]
fn test_word_forward_multiline_bigword() {
    let buffer = make_buffer("hello.world\n  foo");
    let cursor = make_cursor(0, 0);

    let pos = MotionEngine::calculate(
        &buffer,
        &cursor,
        Motion::Word {
            direction: Direction::Forward,
            boundary: WordBoundary::BigWord,
            end: false,
        },
        1,
    );
    // BigWord "hello.world" -> skip to next line -> "foo"
    assert_eq!(pos, Some(Position::new(1, 2)));
}

// === Coverage: word_backward from col 0 wrapping to previous line ===

#[test]
fn test_word_backward_wrap_previous_line() {
    let buffer = make_buffer("hello world\nfoo");
    let cursor = make_cursor(1, 0); // at start of "foo"

    let pos = MotionEngine::calculate(
        &buffer,
        &cursor,
        Motion::Word {
            direction: Direction::Backward,
            boundary: WordBoundary::Word,
            end: false,
        },
        1,
    );
    assert_eq!(pos, Some(Position::new(0, 6))); // "world"
}

// === Coverage: word_forward past buffer end (L199-200) ===

#[test]
fn test_word_forward_past_buffer_end_bigword() {
    // Use BigWord so we skip the entire last word as non-whitespace,
    // then when we can't advance to the next line, the loop iterates
    // with pos.line incremented past line_count, hitting L198-200 clamp.
    let buffer = make_buffer("hello\nworld");
    let cursor = make_cursor(1, 0); // on "world" (last line)

    let pos = MotionEngine::calculate(
        &buffer,
        &cursor,
        Motion::Word {
            direction: Direction::Forward,
            boundary: WordBoundary::BigWord,
            end: false,
        },
        1,
    );
    // Should clamp to last position of the last line
    assert!(pos.is_some());
    let p = pos.unwrap();
    assert_eq!(p.line, 1);
    assert_eq!(p.column, 4); // "world" has 5 chars, last col = 4
}

// === Coverage: word_end at last line boundary (L288-289, L308-310) ===

#[test]
fn test_word_end_at_last_line_boundary_trailing_spaces() {
    // Buffer with trailing whitespace on last line: after moving forward one
    // position from end of word, we land on whitespace. The loop skips
    // whitespace, reaches end of last line (x >= chars.len()), and since
    // there's no next line, hits L308-310 clamping to end of last line.
    let buffer = make_buffer("hello\nworld   ");
    let cursor = make_cursor(1, 4); // on 'd' of "world"

    let pos = MotionEngine::calculate(
        &buffer,
        &cursor,
        Motion::Word {
            direction: Direction::Forward,
            boundary: WordBoundary::Word,
            end: true,
        },
        1,
    );
    // Should stay/clamp within last line when trailing spaces exhaust
    assert!(pos.is_some());
    let p = pos.unwrap();
    assert_eq!(p.line, 1);
}

// === Coverage: word_backward multi punctuation (L402-403) ===

#[test]
fn test_word_backward_multi_punctuation() {
    // Buffer "foo:::bar", cursor on 'b' (col 6), word_backward
    // Backward from 'b' lands on ':' at x=5, which is punctuation.
    // The loop at L398-403 traverses backward through the ':::' cluster.
    let buffer = make_buffer("foo:::bar");
    let cursor = make_cursor(0, 6); // on 'b'

    let pos = MotionEngine::calculate(
        &buffer,
        &cursor,
        Motion::Word {
            direction: Direction::Backward,
            boundary: WordBoundary::Word,
            end: false,
        },
        1,
    );
    assert_eq!(pos, Some(Position::new(0, 3))); // start of ':::'
}

// === Coverage: find_forward_bracket no closing match (L749) ===

#[test]
fn test_find_forward_bracket_no_closing_match() {
    // Buffer with an opening bracket but no matching close.
    // find_forward_bracket scans to end of buffer without finding
    // close, falls through to return None at L749.
    let buffer = make_buffer("(hello world");
    let cursor = make_cursor(0, 0); // on '('

    let pos = MotionEngine::calculate(&buffer, &cursor, Motion::MatchBracket, 1);
    assert_eq!(pos, None);
}

// === Coverage: backward bracket nested depth (L768) ===

#[test]
fn test_backward_bracket_nested_depth_on_first_line() {
    // Buffer "(())", cursor on closing ')' at col 3.
    // find_backward_bracket scans backward from col 2:
    //   col 2: ')' -> depth 1->2 (hits L768!)
    //   col 1: '(' -> depth 2->1
    //   col 0: '(' -> depth 1->0 -> match!
    let buffer = make_buffer("(())");
    let cursor = make_cursor(0, 3); // on outer closing ')'

    let pos = MotionEngine::calculate(&buffer, &cursor, Motion::MatchBracket, 1);
    assert_eq!(pos, Some(Position::new(0, 0))); // matches outer '('
}

// === Coverage: word_forward overshoot past buffer end (L199-201) ===

#[test]
fn test_word_forward_overshoot_past_buffer_end() {
    // High count on short buffer: word_forward loops past last line,
    // triggering pos.line >= line_count guard (L198-201).
    let buffer = make_buffer("a b");
    let cursor = make_cursor(0, 0);
    let pos = MotionEngine::calculate(
        &buffer,
        &cursor,
        Motion::Word {
            direction: Direction::Forward,
            boundary: WordBoundary::Word,
            end: false,
        },
        10,
    );
    assert!(pos.is_some());
}

// === Coverage: word_forward next line all whitespace (L261) ===

#[test]
fn test_word_forward_next_line_all_whitespace() {
    // Buffer where next line is entirely whitespace — word_forward skips it
    // and falls through (nx reaches end without finding non-whitespace).
    let buffer = make_buffer("hello\n   \n   ");
    let cursor = make_cursor(0, 4);
    let pos = MotionEngine::calculate(
        &buffer,
        &cursor,
        Motion::Word {
            direction: Direction::Forward,
            boundary: WordBoundary::Word,
            end: false,
        },
        1,
    );
    // Should clamp to some position (end of buffer)
    assert!(pos.is_some());
}

// === Coverage: word_end overshoot past buffer end (L288-290) ===

#[test]
fn test_word_end_overshoot_past_buffer_end() {
    let buffer = make_buffer("a b");
    let cursor = make_cursor(0, 0);
    let pos = MotionEngine::calculate(
        &buffer,
        &cursor,
        Motion::Word {
            direction: Direction::Forward,
            boundary: WordBoundary::Word,
            end: true,
        },
        10,
    );
    assert!(pos.is_some());
}

// === Coverage: word_end backward on empty line (L451-452) ===

#[test]
fn test_word_end_backward_on_empty_first_line() {
    // Empty first line: word_end backward hits empty chars, line 0 -> break (L452).
    let buffer = make_buffer("\nhello");
    let cursor = make_cursor(0, 0);
    let pos = MotionEngine::calculate(
        &buffer,
        &cursor,
        Motion::Word {
            direction: Direction::Backward,
            boundary: WordBoundary::Word,
            end: true,
        },
        1,
    );
    // Should handle gracefully (stay or clamp)
    let _ = pos;
}

// === Coverage: paragraph forward past end (L539-540) ===

#[test]
fn test_paragraph_forward_past_end_of_buffer() {
    // Buffer where paragraph motion with high count overshoots.
    // After skipping empty lines, current_line >= line_count -> break (L540).
    let buffer = make_buffer("hello\n\nworld\n\n");
    let cursor = make_cursor(0, 0);
    let pos = MotionEngine::calculate(&buffer, &cursor, Motion::Paragraph(Direction::Forward), 10);
    assert!(pos.is_some());
}

// === Coverage: match_bracket cursor not on bracket (L700) ===

#[test]
fn test_match_bracket_cursor_not_on_any_bracket() {
    // Cursor on 'x', no brackets anywhere on line.
    // Falls through cursor_char bracket check at L700.
    let buffer = make_buffer("hello");
    let cursor = make_cursor(0, 2);
    let pos = MotionEngine::calculate(&buffer, &cursor, Motion::MatchBracket, 1);
    assert_eq!(pos, None);
}

// === Coverage: word_forward overshoot guard (L199-201) ===

#[test]
fn test_word_forward_high_count_two_line_buffer() {
    // Two-line buffer with high count. word_forward is called 10 times
    // in the count loop. After reaching the end, repeated calls should
    // clamp correctly.
    let buffer = make_buffer("ab\ncd");
    let cursor = make_cursor(0, 0);
    let pos = MotionEngine::calculate(
        &buffer,
        &cursor,
        Motion::Word {
            direction: Direction::Forward,
            boundary: WordBoundary::Word,
            end: false,
        },
        10,
    );
    assert!(pos.is_some());
    let p = pos.unwrap();
    assert_eq!(p.line, 1);
}

#[test]
fn test_word_forward_bigword_high_count_small_buffer() {
    // Single-word lines with BigWord boundary and high count.
    // After exhausting all lines, the overshoot guard should clamp.
    let buffer = make_buffer("x\ny");
    let cursor = make_cursor(0, 0);
    let pos = MotionEngine::calculate(
        &buffer,
        &cursor,
        Motion::Word {
            direction: Direction::Forward,
            boundary: WordBoundary::BigWord,
            end: false,
        },
        20,
    );
    assert!(pos.is_some());
    let p = pos.unwrap();
    assert_eq!(p.line, 1);
    assert_eq!(p.column, 0);
}

// === Coverage: word_forward next line all whitespace loop continuation (L261) ===

#[test]
fn test_word_forward_multiple_whitespace_lines_then_content() {
    // Buffer with word, then two whitespace-only lines, then content.
    // word_forward from end of first word should skip both whitespace
    // lines (hitting L261 fallthrough twice) before landing on content.
    let buffer = make_buffer("hello\n   \n   \nworld");
    let cursor = make_cursor(0, 4);
    let pos = MotionEngine::calculate(
        &buffer,
        &cursor,
        Motion::Word {
            direction: Direction::Forward,
            boundary: WordBoundary::Word,
            end: false,
        },
        1,
    );
    assert_eq!(pos, Some(Position::new(3, 0)));
}

#[test]
fn test_word_forward_bigword_whitespace_line_fallthrough() {
    // BigWord variant: buffer where next line is entirely whitespace.
    // After skipping the BigWord and whitespace on current line, next line
    // is all whitespace, triggering L261 fallthrough.
    let buffer = make_buffer("abc\n   \ndef");
    let cursor = make_cursor(0, 0);
    let pos = MotionEngine::calculate(
        &buffer,
        &cursor,
        Motion::Word {
            direction: Direction::Forward,
            boundary: WordBoundary::BigWord,
            end: false,
        },
        1,
    );
    assert_eq!(pos, Some(Position::new(2, 0)));
}

// === Coverage: word_end overshoot guard (L288-290) ===

#[test]
fn test_word_end_high_count_two_line_buffer() {
    // Two-line buffer with high count for word_end. After reaching the
    // last word end, repeated calls should clamp via the guard.
    let buffer = make_buffer("ab\ncd");
    let cursor = make_cursor(0, 0);
    let pos = MotionEngine::calculate(
        &buffer,
        &cursor,
        Motion::Word {
            direction: Direction::Forward,
            boundary: WordBoundary::Word,
            end: true,
        },
        10,
    );
    assert!(pos.is_some());
    let p = pos.unwrap();
    assert_eq!(p.line, 1);
}

#[test]
fn test_word_end_bigword_high_count_small_buffer() {
    // BigWord end motion with high count on a 2-line buffer.
    let buffer = make_buffer("x\ny");
    let cursor = make_cursor(0, 0);
    let pos = MotionEngine::calculate(
        &buffer,
        &cursor,
        Motion::Word {
            direction: Direction::Forward,
            boundary: WordBoundary::BigWord,
            end: true,
        },
        20,
    );
    assert!(pos.is_some());
    let p = pos.unwrap();
    assert_eq!(p.line, 1);
    assert_eq!(p.column, 0);
}

// === Coverage: word_backward punctuation single char (L405-406) ===

#[test]
fn test_word_backward_single_punctuation() {
    // Buffer "a.b", cursor on 'b' (col 2). word_backward moves to col 1
    // which is '.', a single punctuation character. The while loop at L398
    // checks chars.get(0)='a' which is alphanumeric, so loop exits
    // immediately. This exercises the else-if branch closing braces L405-406.
    let buffer = make_buffer("a.b");
    let cursor = make_cursor(0, 2);
    let pos = MotionEngine::calculate(
        &buffer,
        &cursor,
        Motion::Word {
            direction: Direction::Backward,
            boundary: WordBoundary::Word,
            end: false,
        },
        1,
    );
    assert_eq!(pos, Some(Position::new(0, 1)));
}

#[test]
fn test_word_backward_punctuation_at_line_start() {
    // Buffer ".hello", cursor at col 1 ('h'). word_backward: x = 0,
    // chars.get(0) = '.', which is punctuation. x == 0, so the while
    // loop at L398 doesn't execute. Closing braces L405-406 are hit.
    let buffer = make_buffer(".hello");
    let cursor = make_cursor(0, 1);
    let pos = MotionEngine::calculate(
        &buffer,
        &cursor,
        Motion::Word {
            direction: Direction::Backward,
            boundary: WordBoundary::Word,
            end: false,
        },
        1,
    );
    assert_eq!(pos, Some(Position::new(0, 0)));
}

#[test]
fn test_word_backward_through_mixed_punct_word() {
    // Buffer "foo.bar{baz", cursor at col 8 ('b' of baz).
    // word_backward: x=7 ('{'), punctuation. While loop checks
    // chars.get(6)='r' which is alphanumeric -> exit. pos.column=7.
    let buffer = make_buffer("foo.bar{baz");
    let cursor = make_cursor(0, 8);
    let pos = MotionEngine::calculate(
        &buffer,
        &cursor,
        Motion::Word {
            direction: Direction::Backward,
            boundary: WordBoundary::Word,
            end: false,
        },
        1,
    );
    assert_eq!(pos, Some(Position::new(0, 7)));
}

// === Coverage: paragraph search fallthrough (L700) ===

#[test]
fn test_match_bracket_non_bracket_char_with_bracket_ahead() {
    // Cursor on a non-bracket character 'x' but there IS a bracket
    // later on the line. Falls through the cursor_char check at L700
    // (none of the BRACKET_PAIRS match 'x'), then the forward search
    // on the rest of the line finds the bracket.
    let buffer = make_buffer("x(hello)");
    let cursor = make_cursor(0, 0);
    let pos = MotionEngine::calculate(&buffer, &cursor, Motion::MatchBracket, 1);
    assert_eq!(pos, Some(Position::new(0, 7)));
}

#[test]
fn test_match_bracket_letter_before_bracket() {
    // Cursor on 'a', not a bracket. Falls through L700 to forward search.
    let buffer = make_buffer("abc{def}");
    let cursor = make_cursor(0, 1);
    let pos = MotionEngine::calculate(&buffer, &cursor, Motion::MatchBracket, 1);
    assert_eq!(pos, Some(Position::new(0, 7)));
}

// === Coverage: backward bracket loop continuation at col 0 (L777) ===

#[test]
fn test_backward_bracket_close_at_col_zero() {
    // Closing bracket at column 0 of a line. find_backward_bracket
    // is called with start=(line, 0). start.column == 0, so the inner
    // `if start.column > 0` block is skipped but the `if let` at L762
    // still executes, hitting L777.
    let buffer = make_buffer("(hello\n)");
    let cursor = make_cursor(1, 0); // on ')' at column 0
    let pos = MotionEngine::calculate(&buffer, &cursor, Motion::MatchBracket, 1);
    assert_eq!(pos, Some(Position::new(0, 0)));
}

#[test]
fn test_backward_bracket_nested_col_zero() {
    // Nested case: closing bracket at column 0 with nested brackets
    // on previous lines. The first-line handling at L762-776 runs with
    // start.column=0, skipping the inner loop, then the previous-lines
    // loop finds the match.
    let buffer = make_buffer("(\n  (inner)\n)");
    let cursor = make_cursor(2, 0); // on ')' at col 0
    let pos = MotionEngine::calculate(&buffer, &cursor, Motion::MatchBracket, 1);
    assert_eq!(pos, Some(Position::new(0, 0)));
}

// === Coverage: paragraph motion edge cases ===

#[test]
fn test_paragraph_forward_single_paragraph_no_blank() {
    // Buffer with only non-empty lines and no blank separator.
    // paragraph_motion forward: skips current paragraph (all lines),
    // then current_line >= line_count, so found stays 0.
    // Returns position at last line.
    let buffer = make_buffer("aaa\nbbb\nccc");
    let cursor = make_cursor(0, 0);
    let pos = MotionEngine::calculate(&buffer, &cursor, Motion::Paragraph(Direction::Forward), 1);
    assert!(pos.is_some());
    let p = pos.unwrap();
    assert_eq!(p.line, 2);
}

#[test]
fn test_paragraph_backward_multiple_paragraphs() {
    // Three paragraphs separated by blank lines. From the last paragraph
    // with count=2, should skip back two paragraphs to the first.
    let buffer = make_buffer("aaa\nbbb\n\nccc\nddd\n\neee\nfff");
    let cursor = make_cursor(7, 0); // on "fff"
    let pos = MotionEngine::calculate(&buffer, &cursor, Motion::Paragraph(Direction::Backward), 2);
    assert!(pos.is_some());
    let p = pos.unwrap();
    // Paragraph backward with count=2 lands at start of first paragraph
    assert_eq!(p.line, 0);
}

#[test]
fn test_paragraph_backward_high_count() {
    // High count backward paragraph motion should clamp to line 0.
    let buffer = make_buffer("aaa\n\nbbb\n\nccc");
    let cursor = make_cursor(4, 0);
    let pos = MotionEngine::calculate(&buffer, &cursor, Motion::Paragraph(Direction::Backward), 10);
    assert!(pos.is_some());
    assert_eq!(pos.unwrap().line, 0);
}

// ============================================================
// b9_repro: word_end_backward ignores WordBoundary parameter.
// ge and gE behave identically because _boundary is unused.
//
// Moved from core/motion/engine.rs during Phase 5 kernel cleanup (#740).
// ============================================================

fn ge(buf: &SimpleText, line: usize, col: usize) -> Option<Position> {
    let cursor = Cursor::new(Position::new(line, col));
    MotionEngine::calculate(
        buf,
        &cursor,
        Motion::Word {
            direction: Direction::Backward,
            boundary: WordBoundary::Word,
            end: true,
        },
        1,
    )
}

fn g_e(buf: &SimpleText, line: usize, col: usize) -> Option<Position> {
    let cursor = Cursor::new(Position::new(line, col));
    MotionEngine::calculate(
        buf,
        &cursor,
        Motion::Word {
            direction: Direction::Backward,
            boundary: WordBoundary::BigWord,
            end: true,
        },
        1,
    )
}

// === ge: "foo::bar baz" ===

#[test]
fn ge_from_baz_stops_at_bar_end() {
    let buf = SimpleText::new("foo::bar baz");
    let pos = ge(&buf, 0, 9).unwrap();
    assert_eq!(pos.column, 7, "ge from baz should stop at 'r' in bar");
}

#[test]
fn g_e_from_baz_stops_at_bar_end() {
    let buf = SimpleText::new("foo::bar baz");
    let pos = g_e(&buf, 0, 9).unwrap();
    assert_eq!(pos.column, 7, "gE from baz should stop at 'r' in bar");
}

// === ge: "foo::bar" from col 7 ===

#[test]
fn ge_from_bar_end_stops_at_colon() {
    let buf = SimpleText::new("foo::bar");
    let pos = ge(&buf, 0, 7).unwrap();
    assert_eq!(pos.column, 4, "ge from bar end should stop at ':' (punct end)");
}

#[test]
fn g_e_from_bar_end_inside_word() {
    let buf = SimpleText::new("foo::bar");
    let pos = g_e(&buf, 0, 7).unwrap();
    assert_eq!(
        pos.column, 0,
        "gE from bar end in 'foo::bar' should go to col 0 (prev line end or start)"
    );
}

// === Coverage: Phase 3 punct-skip branch in ge (WordBoundary::Word) ===

#[test]
fn ge_punct_skip_stops_at_underscore() {
    let buf = SimpleText::new("_..a");
    let pos = ge(&buf, 0, 2).unwrap();
    assert_eq!(pos.column, 0, "ge punct skip should stop at underscore");
}

// === Coverage: L504:br1 — punct skip loop not entered (x==0) ===

#[test]
fn ge_punct_skip_at_col_zero() {
    let buf = SimpleText::new("..");
    let pos = ge(&buf, 0, 1).unwrap();
    assert_eq!(pos.column, 0, "ge punct skip at col 0 should stay at 0");
}
