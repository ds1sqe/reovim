//! Positions that survive edits: a line and a column, never a byte
//! offset into the text, one Buffer's local table of them, and
//! `moved`, the one door through which every edit reaches them
//! (settlement 3 of 01-rung3.md; 01.1-marks.md).
//!
//! The bytes an edit removed are gone by the time `Vim::marked` sees
//! it, so a mark cannot be found by re-reading the text: `starts`
//! keeps each line's own first byte, as it stood before the edit,
//! and `moved` reads it, then brings it up to date for the next one.

use alloc::collections::BTreeMap;
use alloc::vec::Vec;

use reovim_kernel_interface::Rd;

use crate::Edit;

/// A line and a column, both from 0, the column in bytes.
///
/// How nvim keeps a position (`:help getpos`), and so how `moved`
/// keeps one too, one unit with the edits it moves through, which are
/// bytes themselves.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Pos {
    pub line: usize,
    pub column: usize,
}

/// One Buffer's marks: the local table, `a`-`z` and `' [ ] < > .`,
/// and the index `moved` reads and keeps up to date.
#[derive(Debug)]
pub struct Marks {
    pub(crate) local: BTreeMap<char, Pos>,
    /// Every line's own start: `{0}` and, for each `\n` in the text,
    /// the byte after it — a final `\n` included, so a text that
    /// ends with one has one more entry than it has lines.
    starts: Vec<usize>,
    /// The text's own length, kept apart from `starts`: the text's
    /// last line may have no `\n` of its own to make its end an
    /// entry there.
    length: usize,
}

impl Default for Marks {
    /// The index of an empty text: one start, at byte 0. `starts`
    /// must never be empty; `reindex_after` only ever carries a `0`
    /// through, never invents one.
    fn default() -> Self {
        Self { local: BTreeMap::new(), starts: alloc::vec![0], length: 0 }
    }
}

/// `starts` and `length` together, `moved`'s own two threaded through
/// every free function below it as one argument, short enough at
/// each call's own line to stay a plain expression, not a block: the
/// nested block a call needing its own would be is one past what
/// clippy's `excessive_nesting` allows in `moved`.
type Index<'a> = (&'a [usize], usize);

/// The line byte `at` falls on.
fn line_of(starts: &[usize], at: usize) -> usize {
    starts.partition_point(|&s| s <= at).saturating_sub(1)
}

/// `pos`, an old position, moved by `edit` over `index` (`starts`
/// with `length` after them): `None` when a line it took removes it,
/// the new position otherwise, unchanged when its line is past what
/// `starts` describes (a file mark of a Buffer not yet read whole).
/// A column never moves for an edit on its own line, nvim's rule; a
/// later line merged into it instead has its column found afresh,
/// the merge arm below.
fn move_one(index: Index<'_>, pos: Pos, edit: &Edit) -> Option<Pos> {
    let (starts, length) = index;
    let Some(&start) = starts.get(pos.line) else { return Some(pos) };
    let end = starts.get(pos.line + 1).copied().unwrap_or(length);
    let reach = edit.at + edit.len;
    if end <= edit.at {
        return Some(pos);
    }
    // Past the edit: a line whose own start the edit does not reach,
    // or one an insert of nothing lands exactly on (which it pushes
    // forward, never merges into); a removal that reaches exactly to
    // a line's start takes the `\n` before it and must reach the
    // merge arm below instead.
    if start > reach || (edit.len == 0 && start == reach) {
        let inserted = edit.text.matches('\n').count();
        let removed = starts
            .iter()
            .skip(1)
            .filter(|&&s| edit.at < s && s <= reach)
            .count();
        let new_line = (pos.line + inserted).saturating_sub(removed);
        return Some(Pos { line: new_line, ..pos });
    }
    // A line has its own `\n` unless it is the text's very last and
    // the text does not end in one, the one case with no `starts`
    // entry after its own.
    let taken = if pos.line + 1 < starts.len() {
        edit.at <= start && end <= reach
    } else {
        edit.at < start && end <= reach
    };
    if taken {
        return None;
    }
    if start <= edit.at {
        return Some(pos);
    }
    let inserted = edit.text.matches('\n').count();
    let start_line = line_of(starts, edit.at);
    // `prefix` is the edit's own start line's own bytes before it,
    // only real when the merged line stays that one: a `\n` inside
    // `edit.text` starts a line of its own, with no bytes of the
    // edit's start line before it at all.
    let prefix = if inserted == 0 { edit.at - starts[start_line] } else { 0 };
    let boundary = reach.saturating_sub(start);
    let tail = edit
        .text
        .rfind('\n')
        .map_or(edit.text.len(), |i| edit.text.len() - i - 1);
    let column = (prefix + tail + pos.column).saturating_sub(boundary);
    Some(Pos { line: start_line + inserted, column })
}

/// A local mark's own place in `local.retain`: moved with `edit` and
/// kept, dropped for a letter a wholly taken line removed, or
/// relocated to the edit's own line for `[ ] < > . '`, which `gv` and
/// a range end need when the line they named is gone.
fn retain_local(
    index: Index<'_>,
    name: char,
    pos: &mut Pos,
    edit: &Edit,
) -> bool {
    if let Some(new) = move_one(index, *pos, edit) {
        *pos = new;
        return true;
    }
    if name.is_ascii_lowercase() {
        return false;
    }
    pos.line = line_of(index.0, edit.at);
    true
}

/// A file mark's own place in `files.retain`: another source's left
/// alone, this one's moved with `edit` and kept, or dropped when the
/// line it named was wholly taken.
fn retain_file(
    index: Index<'_>,
    source: Rd,
    rd: Rd,
    pos: &mut Pos,
    edit: &Edit,
) -> bool {
    if rd != source {
        return true;
    }
    let Some(new) = move_one(index, *pos, edit) else { return false };
    *pos = new;
    true
}

/// Every file mark of `source`, moved with `edit` and kept: a free
/// function, not `moved`'s own `if let Some(source)` block, whose
/// nesting there is one past what clippy's `excessive_nesting` allows.
fn retain_files(
    index: Index<'_>,
    source: Rd,
    files: &mut BTreeMap<char, (Rd, Pos)>,
    edit: &Edit,
) {
    files.retain(|_, (rd, pos)| retain_file(index, source, *rd, pos, edit));
}

impl Marks {
    /// Every local mark and, of `files`, every one whose source is
    /// `source`, moved by `edit`; `starts` and `length` brought up to
    /// date after.
    pub(crate) fn moved(
        &mut self,
        edit: &Edit,
        files: &mut BTreeMap<char, (Rd, Pos)>,
        source: Option<Rd>,
    ) {
        let Self { local, starts, length } = self;
        let index = (starts.as_slice(), *length);
        local.retain(|&name, pos| retain_local(index, name, pos, edit));
        if let Some(source) = source {
            retain_files(index, source, files, edit);
        }
        self.reindex_after(edit);
    }

    /// `starts` and `length` after `edit`: every line start before it
    /// unchanged, the ones `edit.text` itself starts spliced in, then
    /// every one after shifted by what it put in less what it took.
    /// Exact by construction: every real start the text has is
    /// always here, a final `\n` included, so nothing is ever
    /// rebuilt from scratch after this.
    fn reindex_after(&mut self, edit: &Edit) {
        let at = edit.at;
        let mut next: Vec<usize> =
            self.starts.iter().copied().take_while(|&s| s <= at).collect();
        let inserted = edit.text.match_indices('\n').map(|(i, _)| at + i + 1);
        next.extend(inserted);
        let tail = self.starts.iter().copied().filter(|&s| s > at + edit.len);
        next.extend(tail.map(|s| s - edit.len + edit.text.len()));
        self.starts = next;
        self.length = self.length - edit.len + edit.text.len();
    }
}

#[cfg(test)]
mod tests {
    use alloc::collections::BTreeMap;
    use alloc::string::String;
    use alloc::vec;
    use alloc::vec::Vec;

    use reovim_kernel_interface::Rd;

    use super::{Marks, Pos, move_one};
    use crate::Edit;

    fn edit(at: usize, len: usize, text: &str) -> Edit {
        Edit { at, len, text: String::from(text), join: false }
    }

    /// The starts and length `text` has, built fresh from it, the
    /// way `moved` would leave them for the same text.
    fn fresh(text: &str) -> (Vec<usize>, usize) {
        let mut starts = vec![0];
        let mut at = 0;
        while let Some(i) = text[at..].find('\n') {
            at += i + 1;
            starts.push(at);
        }
        (starts, text.len())
    }

    fn at(line: usize, column: usize) -> Pos {
        Pos { line, column }
    }

    /// `text`, `sets` as its starting local marks, moved by `edit`;
    /// its `starts` and `length`, read straight off it, must equal a
    /// fresh build of the text `edit` makes, on every case.
    fn moved_local(text: &str, sets: &[(char, Pos)], edit: &Edit) -> Marks {
        let (starts, length) = fresh(text);
        let mut marks = Marks { starts, length, ..Marks::default() };
        for &(name, pos) in sets {
            marks.local.insert(name, pos);
        }
        let mut files = BTreeMap::new();
        marks.moved(edit, &mut files, None);
        let mut after = String::from(text);
        after.replace_range(edit.at..edit.at + edit.len, &edit.text);
        let want = fresh(&after);
        assert_eq!(
            (marks.starts.clone(), marks.length),
            want,
            "index vs fresh"
        );
        marks
    }

    #[test]
    fn an_insert_before_the_marks_column_keeps_the_column() {
        let marks =
            moved_local("one two\n", &[('a', at(0, 4))], &edit(0, 0, "ab "));
        assert_eq!(marks.local[&'a'], at(0, 4), "the column is literal");
    }

    #[test]
    fn a_removal_that_starts_the_marks_own_line_keeps_the_column_too() {
        // "one two\n", a delete at the line's own start: nvim's own
        // column never moves for an edit on the mark's own line,
        // removal included, not only an insert.
        let marks =
            moved_local("one two\n", &[('a', at(0, 4))], &edit(0, 1, ""));
        assert_eq!(marks.local[&'a'], at(0, 4), "the column is literal");
    }

    #[test]
    fn a_line_put_above_shifts_the_mark_down() {
        let marks =
            moved_local("a\nb\n", &[('a', at(1, 0))], &edit(0, 0, "new\n"));
        assert_eq!(marks.local[&'a'], at(2, 0));
    }

    #[test]
    fn a_line_put_below_the_last_line_shifts_nothing_after_it() {
        // "a\nb\n" plus "c\n" appended at the text's own end: the mark
        // on "b" is untouched, and the index gains "c"'s start.
        let marks =
            moved_local("a\nb\n", &[('a', at(1, 0))], &edit(4, 0, "c\n"));
        assert_eq!(marks.local[&'a'], at(1, 0));
        assert_eq!((marks.starts, marks.length), (vec![0, 2, 4, 6], 6));
    }

    #[test]
    fn dd_of_the_marked_line_takes_it() {
        let marks =
            moved_local("a\nb\nc\n", &[('a', at(1, 0))], &edit(2, 2, ""));
        assert!(!marks.local.contains_key(&'a'));
    }

    #[test]
    fn dd_of_the_last_line_with_an_eol_takes_its_mark() {
        // "a\nb\nc\nd\n": `d`'s own `\n` is the edit's own end, not a
        // "last line" special case keyed off position.
        let marks =
            moved_local("a\nb\nc\nd\n", &[('a', at(3, 0))], &edit(6, 2, ""));
        assert!(!marks.local.contains_key(&'a'));
    }

    #[test]
    fn dd_of_the_last_line_with_no_eol_takes_the_newline_before_it_too() {
        // "a\nb\nc", no final `\n`: `dd` of "c" removes the `\n`
        // before it instead of one of its own, as `whole_lines` does.
        let marks = moved_local("a\nb\nc", &[('a', at(2, 0))], &edit(3, 2, ""));
        assert!(!marks.local.contains_key(&'a'));
    }

    #[test]
    fn a_removal_reaching_exactly_to_a_later_lines_start_merges_it() {
        // "one\ntwo\n", backspace at "two"'s own start: `Edit{3,1,""}`
        // takes the `\n` alone. The mark on "two"'s 'w' (1,1) must
        // reach the merge arm, not read as "past the edit".
        let marks =
            moved_local("one\ntwo\n", &[('a', at(1, 1))], &edit(3, 1, ""));
        assert_eq!(marks.local[&'a'], at(0, 4), "onto 'w', now in line 0");
    }

    #[test]
    fn a_charwise_delete_across_three_lines_moves_what_survives() {
        // "one\ntwo\nthree\n": from byte 3 ("one"'s own `\n`) through
        // byte 10 exclusive takes "\ntwo\nth" whole, "one" and
        // "ree\n" left joined as one line.
        let marks = moved_local(
            "one\ntwo\nthree\n",
            &[('m', at(1, 1)), ('e', at(2, 2))],
            &edit(3, 7, ""),
        );
        assert!(!marks.local.contains_key(&'m'), "the wholly taken middle");
        assert_eq!(marks.local[&'e'], at(0, 3), "the tail, merged in");
    }

    #[test]
    fn a_put_below_shifts_a_mark_on_a_later_line() {
        let marks =
            moved_local("a\nb\nc\n", &[('a', at(2, 0))], &edit(2, 0, "new\n"));
        assert_eq!(marks.local[&'a'], at(3, 0));
    }

    #[test]
    fn a_special_mark_taken_with_its_line_lands_on_the_edits_first_line() {
        // `[` on the deleted middle line goes to the edit's own start
        // line instead of vanishing, unlike a letter mark.
        let marks = moved_local(
            "a\nb\nc\n",
            &[('[', at(1, 0)), ('a', at(1, 0))],
            &edit(2, 2, ""),
        );
        assert_eq!(marks.local[&'['], at(1, 0), "kept, on the edit's line");
        assert!(!marks.local.contains_key(&'a'), "a letter mark is taken");
    }

    #[test]
    fn a_join_seam_moves_a_mark_on_the_second_line_to_where_it_joins() {
        // "one\ntwo\n": a `J`-shaped seam edit replacing the `\n`
        // ending "one" with a space.
        let marks =
            moved_local("one\ntwo\n", &[('a', at(1, 1))], &edit(3, 1, " "));
        assert_eq!(marks.local[&'a'], at(0, 5), "onto the 'w' of two");
    }

    #[test]
    fn a_multiline_edit_finds_the_tail_on_its_own_last_line_not_the_first() {
        // "one\ntwo\nthree\n", `Edit{2,4,"X\nY"}` replaces "e\ntw" with
        // "X\nY": the mark's own 'o' (two's own) survives as "Yo"'s
        // second char, on the line the edit's `\n` starts, not on
        // "onX", the edit's own first line, which has no part of it.
        let marks = moved_local(
            "one\ntwo\nthree\n",
            &[('a', at(1, 2))],
            &edit(2, 4, "X\nY"),
        );
        assert_eq!(marks.local[&'a'], at(1, 1), "onto 'o', now on Yo");
    }

    #[test]
    fn a_join_seam_across_multibyte_lines_moves_the_column_in_bytes() {
        // "é\nçxyz\n": `é` and `ç` are two bytes each, so a char count
        // would be wrong here where a byte offset is not. The seam
        // (`é`'s own `\n` replaced by a space) joins them into
        // "é çxyz"; the mark on "çxyz"'s 'y' (byte column 3: `ç` 2,
        // `x` 1) survives onto the joined line's own 'y' (byte column
        // 6: `é` 2, the space 1, `ç` 2, `x` 1).
        let marks =
            moved_local("é\nçxyz\n", &[('a', at(1, 3))], &edit(2, 1, " "));
        assert_eq!(marks.local[&'a'], at(0, 6), "onto 'y', now on one line");
    }

    #[test]
    fn the_synthetic_edit_of_a_reload_moves_marks_by_its_span() {
        // A reload replaces the whole differing middle; a mark past
        // it shifts by the lines the new middle adds.
        let marks = moved_local(
            "one\ntwo\nthree\n",
            &[('a', at(2, 0))],
            &edit(4, 4, "TWO\nAND\n"),
        );
        assert_eq!(marks.local[&'a'], at(3, 0));
    }

    #[test]
    fn an_edit_that_empties_the_text_leaves_one_start() {
        let marks = moved_local("one\n", &[], &edit(0, 4, ""));
        assert_eq!((marks.starts, marks.length), (vec![0], 0));
    }

    #[test]
    fn a_file_mark_past_what_starts_describes_is_left_alone() {
        // A position `starts` (an empty text's own) has no line for:
        // not this edit's to move.
        assert_eq!(
            move_one((&[0], 0), at(3, 2), &edit(0, 0, "x")),
            Some(at(3, 2))
        );
    }

    #[test]
    fn a_file_mark_of_this_source_moves_and_one_of_another_does_not() {
        // `shift_files`'s own retain, not `local`'s: a real `files`
        // table, one entry keyed to `source` and one to another Rd,
        // so a no-op there would still pass every other case here.
        let (starts, length) = fresh("a\nb\nc\n");
        let mut marks = Marks { starts, length, ..Marks::default() };
        let source = Rd(1);
        let elsewhere = Rd(2);
        let mut files = BTreeMap::new();
        files.insert('A', (source, at(2, 0)));
        files.insert('B', (elsewhere, at(2, 0)));
        marks.moved(&edit(0, 0, "new\n"), &mut files, Some(source));
        assert_eq!(files[&'A'], (source, at(3, 0)), "shifted with its source");
        assert_eq!(
            files[&'B'],
            (elsewhere, at(2, 0)),
            "another source, untouched"
        );
    }
}
