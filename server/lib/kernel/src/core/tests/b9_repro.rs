// #720 repro: word_end_backward ignores WordBoundary parameter.
// ge and gE behave identically because _boundary is unused.
//
// Moved from core/motion/engine.rs during Phase 5 kernel cleanup (#740).

use super::*;

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
