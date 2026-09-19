//! The jump list: `<C-o>` and `<Tab>` (Ctrl-I) walk it, one list kept
//! across every Buffer, a hundred entries deep.
//!
//! Every entry moves with the text through [`crate::marks::Marks::moved`],
//! the same door a mark's own position does; this module only holds
//! the list and the two ops `set_previous` and `jumped` drive it
//! through, `push` and `walk`.

use alloc::vec::Vec;

use reovim_kernel_interface::Rd;

use crate::Pos;

/// A hundred entries is what nvim's own `'jumps'` keeps.
const MOST: usize = 100;

/// One place a jump left: the source it names, `None` for a Buffer
/// over nothing that has not gained one yet, and the position it
/// stood at.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Jump {
    pub(crate) source: Option<Rd>,
    pub(crate) pos: Pos,
}

/// The list, and where a walk of it stands: `at == list.len()` is the
/// newest end, the live position, standing on no entry of its own.
#[derive(Debug, Default)]
pub struct Jumps {
    pub(crate) list: Vec<Jump>,
    pub(crate) at: usize,
}

impl Jumps {
    /// `jump`, kept: every entry it names again goes first, and the
    /// oldest is dropped past a hundred. No pop here, unlike `clean`'s:
    /// every jump this takes is a real one, never the cursor's own
    /// stand-in place.
    pub(crate) fn push(&mut self, jump: Jump) {
        self.list.retain(|&j| !same(j, jump));
        self.list.push(jump);
        if self.list.len() > MOST {
            self.list.remove(0);
        }
        self.at = self.list.len();
    }

    /// `back` steps `<C-o>` walks, forward `<Tab>` does, `n` at a
    /// time, `here` the cursor's own place: `clean` runs first, then
    /// the range is checked before anything is pushed, so one out of
    /// range pushes nothing and leaves `at` as it was. In range,
    /// `here` is pushed raw at the newest end; `at` moves and the
    /// entry there is answered.
    pub(crate) fn walk(
        &mut self,
        back: bool,
        n: usize,
        here: Jump,
    ) -> Option<Jump> {
        self.clean(here);
        let len = self.list.len();
        let step =
            if back { self.at.checked_sub(n) } else { self.at.checked_add(n) };
        let mut next = step.filter(|&i| i < len)?;
        if self.at == len {
            self.list.push(here);
        }
        if self.list.len() > MOST {
            self.list.remove(0);
            next = next.checked_sub(1)?;
        }
        self.at = next;
        Some(self.list[next])
    }

    /// nvim's own list cleanup, run at the top of every walk: every
    /// entry a later one of the same source and line repeats is
    /// dropped first, `at` moving down by one for each dropped at or
    /// before it; then, if the list stands at its newest end and its
    /// last entry names `here`'s own source and line too, that entry
    /// — `here` already standing there, not a jump of its own — is
    /// dropped as well.
    fn clean(&mut self, here: Jump) {
        while let Some(i) = self.repeated() {
            self.list.remove(i);
            self.at -= usize::from(i < self.at);
        }
        if self.at == self.list.len()
            && self.list.last().is_some_and(|&l| same(l, here))
        {
            self.list.pop();
            self.at -= 1;
        }
    }

    /// The lowest index some later entry of the same source and line
    /// repeats: a free function, apart for clippy's nesting bound.
    fn repeated(&self) -> Option<usize> {
        (0..self.list.len()).find(|&i| {
            self.list[i + 1..].iter().any(|&l| same(l, self.list[i]))
        })
    }

    /// Every entry with no source yet is `rd`'s now: a Buffer over
    /// nothing that gains one keeps the jumps it made before.
    pub(crate) fn named(&mut self, rd: Rd) {
        let unnamed = self.list.iter_mut().filter(|jump| jump.source.is_none());
        for jump in unnamed {
            jump.source = Some(rd);
        }
    }

    /// Every entry with no source, dropped: nothing names that Buffer
    /// to come back by once it is left. Unnamed entries are always a
    /// suffix of the list: a push only ever adds
    /// one under the current source, and every `leave` forgets them
    /// here, so `at`, always at or past every one of them, only ever
    /// needs clamping to the new, shorter length.
    pub(crate) fn forget_unnamed(&mut self) {
        self.list.retain(|j| j.source.is_some());
        self.at = self.at.min(self.list.len());
    }
}

/// `a` and `b` name the same source and line: `clean`'s and
/// `repeated`'s own notion of a repeat.
fn same(a: Jump, b: Jump) -> bool {
    a.source == b.source && a.pos.line == b.pos.line
}

#[cfg(test)]
mod tests {
    use reovim_kernel_interface::Rd;

    use super::{Jump, Jumps};
    use crate::Pos;

    fn at(line: usize, column: usize) -> Pos {
        Pos { line, column }
    }

    fn jump(source: Option<Rd>, line: usize) -> Jump {
        Jump { source, pos: at(line, 0) }
    }

    #[test]
    fn a_push_takes_the_entry_of_its_line_and_source_and_keeps_anothers() {
        let mut jumps = Jumps::default();
        let a = Some(Rd(1));
        let b = Some(Rd(2));
        jumps.push(jump(a, 3));
        jumps.push(jump(b, 3));
        jumps.push(jump(a, 3));
        assert_eq!(jumps.list, [jump(b, 3), jump(a, 3)], "a's old entry gone");
        assert_eq!(jumps.at, 2, "at the new end");
    }

    #[test]
    fn the_hundred_and_first_drops_the_oldest() {
        let mut jumps = Jumps::default();
        let source = Some(Rd(1));
        for line in 0..101 {
            jumps.push(jump(source, line));
        }
        assert_eq!(jumps.list.len(), 100);
        assert_eq!(jumps.list[0], jump(source, 1), "0 is gone");
        assert_eq!(jumps.list[99], jump(source, 100));
    }

    #[test]
    fn a_walk_back_from_the_newest_end_pushes_here_and_answers_before_it() {
        let mut jumps = Jumps::default();
        let source = Some(Rd(1));
        jumps.push(jump(source, 1));
        jumps.push(jump(source, 2));
        let here = jump(source, 3);
        let found = jumps.walk(true, 1, here);
        assert_eq!(found, Some(jump(source, 2)), "the entry before here");
        assert_eq!(
            jumps.list,
            [jump(source, 1), jump(source, 2), jump(source, 3)],
            "here pushed and kept"
        );
        assert_eq!(jumps.at, 1);
    }

    #[test]
    fn a_walk_s_own_push_is_raw_and_may_duplicate_an_older_line() {
        let mut jumps = Jumps::default();
        let source = Some(Rd(1));
        jumps.push(jump(source, 1));
        jumps.push(jump(source, 2));
        jumps.push(jump(source, 4));
        let here = jump(source, 2); // an older line, not the list's own last
        assert_eq!(jumps.walk(true, 1, here), Some(jump(source, 4)));
        assert_eq!(
            jumps.list,
            [
                jump(source, 1),
                jump(source, 2),
                jump(source, 4),
                jump(source, 2)
            ],
            "here pushed raw: line 2 now stands twice"
        );
    }

    #[test]
    fn a_walk_pops_a_newest_entry_standing_on_heres_own_line() {
        let mut jumps = Jumps::default();
        let source = Some(Rd(1));
        jumps.push(jump(source, 0));
        assert_eq!(jumps.walk(true, 1, jump(source, 0)), None);
        assert!(jumps.list.is_empty(), "the one entry, popped");
        assert_eq!(jumps.at, 0);
    }

    #[test]
    fn a_walk_out_of_range_answers_none_and_pushes_nothing() {
        let mut jumps = Jumps::default();
        let source = Some(Rd(1));
        jumps.push(jump(source, 1));
        jumps.push(jump(source, 2));

        jumps.at = 2;
        assert_eq!(
            jumps.walk(false, 1, jump(source, 9)),
            None,
            "the newest end"
        );
        assert_eq!(jumps.list, [jump(source, 1), jump(source, 2)], "unchanged");
        assert_eq!(jumps.at, 2, "still the newest end");

        jumps.at = 1;
        assert_eq!(
            jumps.walk(true, 2, jump(source, 9)),
            None,
            "past the oldest"
        );
        assert_eq!(jumps.at, 1, "at stands");

        jumps.at = 0;
        assert_eq!(
            jumps.walk(false, 5, jump(source, 9)),
            None,
            "past the newest"
        );
        assert_eq!(jumps.at, 0);
    }

    #[test]
    fn a_refused_walk_from_the_newest_end_pushes_nothing() {
        let mut jumps = Jumps::default();
        let source = Some(Rd(1));
        assert_eq!(jumps.walk(true, 1, jump(source, 0)), None);
        assert!(jumps.list.is_empty(), "nothing pushed");
        assert_eq!(jumps.at, 0);
    }

    #[test]
    fn a_push_from_the_middle_lands_at_the_end_and_cuts_nothing() {
        let mut jumps = Jumps::default();
        let source = Some(Rd(1));
        jumps.push(jump(source, 1));
        jumps.push(jump(source, 2));
        jumps.push(jump(source, 3));
        jumps.at = 0;
        jumps.push(jump(source, 9));
        assert_eq!(
            jumps.list,
            [
                jump(source, 1),
                jump(source, 2),
                jump(source, 3),
                jump(source, 9)
            ],
            "nothing cut"
        );
        assert_eq!(jumps.at, 4, "at the new end");
    }

    #[test]
    fn forgetting_unnamed_entries_drops_them_and_clamps_at() {
        // Unnamed entries are always a suffix: a push only ever adds
        // one under the current source, and every `leave` forgets
        // them here. So `at` on the one named entry needs no
        // clamping, and `at` at the newest end (past the unnamed
        // suffix too) only ever needs it down to the new, shorter end.
        let a = Some(Rd(1));
        let list = || alloc::vec![jump(a, 5), jump(None, 1), jump(None, 3)];

        let mut on_a = Jumps { list: list(), at: 0 };
        on_a.forget_unnamed();
        assert_eq!(on_a.list, [jump(a, 5)]);
        assert_eq!(on_a.at, 0, "already on it, no clamp needed");

        let mut newest = Jumps { list: list(), at: 3 };
        newest.forget_unnamed();
        assert_eq!(newest.list, [jump(a, 5)]);
        assert_eq!(newest.at, 1, "clamped to the new end");
    }
}
