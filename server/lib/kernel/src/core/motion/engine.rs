//! Motion calculation engine — re-exported from `reovim-types-text`.

pub use reovim_types_text::MotionEngine;

// =========================================================================
// #720 repro: word_end_backward ignores WordBoundary parameter.
// ge and gE behave identically because _boundary is unused.
// =========================================================================

#[cfg(test)]
mod b9_repro {
    use {
        super::*,
        crate::mm::{Buffer, Cursor},
    };

    // "foo::bar baz" — '::' is punctuation, separating two words but one WORD.
    // ge from 'b' in "baz" (col 9): should stop at 'r' in "bar" (col 7)
    // for small word (ge), but at 'r' in "bar" (col 7) for big WORD (gE)
    // because the entire "foo::bar" is one WORD.

    use crate::core::{Direction, Motion, WordBoundary};

    fn ge(buf: &Buffer, line: usize, col: usize) -> Option<crate::mm::Position> {
        let cursor = Cursor::new(crate::mm::Position::new(line, col));
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

    fn g_e(buf: &Buffer, line: usize, col: usize) -> Option<crate::mm::Position> {
        let cursor = Cursor::new(crate::mm::Position::new(line, col));
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
        // ge from 'b' in "baz" (col 9).
        // Phase 1: move back → col 8 = ' ' (space). Skip ws → col 7 = 'r'.
        // Phase 2: 'r' at col 7, next='.' ws? no. Word? 'r' is word, ' ' is ws → boundary → yes.
        // Return col 7.
        let buf = Buffer::from_string("foo::bar baz");
        let pos = ge(&buf, 0, 9).unwrap();
        assert_eq!(pos.column, 7, "ge from baz should stop at 'r' in bar");
    }

    #[test]
    fn g_e_from_baz_stops_at_bar_end() {
        // gE from 'b' in "baz" (col 9).
        // Phase 1: move back → col 8 = ' '. Skip ws → col 7 = 'r'.
        // Phase 2: 'r' at col 7, next=' ' → ws → at_word_end = true. Return col 7.
        let buf = Buffer::from_string("foo::bar baz");
        let pos = g_e(&buf, 0, 9).unwrap();
        assert_eq!(pos.column, 7, "gE from baz should stop at 'r' in bar");
    }

    // === ge: "foo::bar" from col 7 ===

    #[test]
    fn ge_from_bar_end_stops_at_colon() {
        // ge from 'r' in "bar" (col 7).
        // Phase 1: move back → col 6 = 'a'. Not ws.
        // Phase 2: 'a' at col 6, next 'r' → both word → same class → not boundary.
        // Phase 3: 'a' is word. Skip word back: col 5 'b' (word) → col 5.
        //   col 4=':' not word → stop. x=5. x>0 → x-=1 → x=4 (':').
        //   ':' not ws → return col 4.
        let buf = Buffer::from_string("foo::bar");
        let pos = ge(&buf, 0, 7).unwrap();
        assert_eq!(pos.column, 4, "ge from bar end should stop at ':' (punct end)");
    }

    #[test]
    fn g_e_from_bar_end_inside_word() {
        // gE from 'r' in "foo::bar" (col 7).
        // Phase 1: move back → col 6 = 'a'. Not ws.
        // Phase 2: 'a' at col 6, next 'r' → both non-ws → BigWord: not boundary.
        // Phase 3: BigWord skip non-ws back → col 0 ('f'). x=0 → break.
        // x==0, pos.line=0 → pos.column=0.
        let buf = Buffer::from_string("foo::bar");
        let pos = g_e(&buf, 0, 7).unwrap();
        assert_eq!(pos.column, 0, "gE from bar end in 'foo::bar' should go to col 0 (prev line end or start)");
    }

    // === Coverage: Phase 3 punct-skip branch in ge (WordBoundary::Word) ===

    #[test]
    fn ge_punct_skip_stops_at_underscore() {
        // "_..a" — ge from 'a' (col 3) with count=2 to force second ge iteration.
        // 1st iteration: move back col 2 ('.'), not ws.
        // Phase 2: chars[2]='.', chars[3]='a': '.' is punct, 'a' is word → boundary → yes.
        // Return col 2.
        //
        // 2nd iteration from col 2:
        // Phase 1: move back col 1 ('.'), not ws.
        // Phase 2: chars[1]='.', chars[2]='.' — same class (both punct) → not end.
        // Phase 3: '.' not word → punct skip:
        //   x=1, chars[0]='_': is_alphanumeric=false, '_'=='_'=true →
        //   !(false || true) = false → exit loop (L506 B2).
        // x=1>0, x-=1→0. chars[0]='_' not ws → pos.column=0.
        let buf = Buffer::from_string("_..a");
        let pos = ge(&buf, 0, 2).unwrap();
        assert_eq!(pos.column, 0, "ge punct skip should stop at underscore");
    }

    // === Coverage: L504:br1 — punct skip loop not entered (x==0) ===

    #[test]
    fn ge_punct_skip_at_col_zero() {
        // Phase 3 punct skip with x=0 → `while x > 0` is false (L504:br1).
        // ".." from col 1. After backing to col 0 ('.').
        // Phase 1: '.' not ws.
        // Phase 2: chars[0]='.', chars[1]='.'. Both punct → same class → not end.
        // Phase 3: '.' not word → punct skip. x=0 → while loop not entered.
        // x==0, pos.line=0 → pos.column=0, break.
        let buf = Buffer::from_string("..");
        let pos = ge(&buf, 0, 1).unwrap();
        assert_eq!(pos.column, 0, "ge punct skip at col 0 should stay at 0");
    }
}
