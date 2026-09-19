//! Vim alone, rung 2: what the corpus cannot see. The revisions an
//! operator makes, the spans a View marks, the messages, `.` over
//! a selection, the registers' kinds.

mod common;

use common::{edit, edits, on, typed};
use reovim_client_api::{MATCH, SELECTION};
use reovim_plugin_vim::{Edit, Effect, Kind, Mode, Register, Vim, Visual};

fn register(text: &str, kind: Kind) -> Register {
    Register { text: text.into(), kind }
}

/// What `/self/vim/bin/substitute` does to the engine, which is what
/// the run of it calls: the Driver and its declaration are the tree's
/// business (tests/ex.rs), and this is the work.
fn subst(vim: &mut Vim, from: usize, to: usize, spec: &str) -> Vec<Edit> {
    let mut all = Vec::new();
    match vim.substitute((from, to), spec) {
        Effect::Edit(edit) => all.push(edit),
        Effect::Edits(more) => all.extend(more),
        _ => {}
    }
    all
}

/// The View's spans of one style, as byte ranges.
fn spans(vim: &mut Vim, rows: usize, style: u64) -> Vec<(u64, u64)> {
    vim.view(rows)
        .spans
        .iter()
        .filter(|span| span.style == style)
        .map(|span| (span.from, span.to))
        .collect()
}

/// What it would select in a region tall enough for anything.
fn selected(vim: &mut Vim) -> Vec<(u64, u64)> {
    spans(vim, 24, SELECTION)
}

/// What it would mark as a search match.
fn marked(vim: &mut Vim) -> Vec<(u64, u64)> {
    spans(vim, 24, MATCH)
}

#[test]
fn an_operator_is_one_revision_and_a_change_types_into_it() {
    let mut vim = on("foo bar\nbaz\n");
    assert_eq!(typed(&mut vim, "dw"), edit(0, 4, "", false));
    assert_eq!(typed(&mut vim, "u"), Effect::Undo, "the tui's to answer");
    assert_eq!(
        edits(&mut vim, "ciwxy<Esc>"),
        vec![
            Edit { at: 0, len: 3, text: String::new(), join: false },
            Edit { at: 0, len: 0, text: "x".into(), join: true },
            Edit { at: 1, len: 0, text: "y".into(), join: true },
        ]
    );
    assert_eq!(vim.text(), "xy\nbaz\n");
    assert_eq!(
        typed(&mut vim, "J"),
        Effect::Edits(vec![Edit {
            at: 2,
            len: 1,
            text: " ".into(),
            join: false
        }])
    );
    assert_eq!(typed(&mut vim, ">>"), edit(0, 6, "\txy baz", false));
    assert_eq!(typed(&mut vim, "wgUiw"), edit(1, 2, "XY", false));
    assert_eq!(typed(&mut vim, "~"), edit(1, 1, "x", false));
}

#[test]
fn many_places_change_as_one_revision() {
    let mut vim = on("a a\na\n");
    let all = subst(&mut vim, 0, 1, "a/b/g");
    assert_eq!(
        all,
        vec![
            Edit { at: 4, len: 1, text: "b".into(), join: false },
            Edit { at: 0, len: 3, text: "b b".into(), join: true },
        ]
    );
    assert_eq!(vim.text(), "b b\nb\n");
    assert_eq!(vim.position(), (1, 0), "on the last line substituted");
    let mut vim = on("ab\ncd\nef\n");
    let all = edits(&mut vim, "l<C-v>jjcx<Esc>");
    assert_eq!(all.len(), 6, "three deletes, the typing, two copies: {all:?}");
    assert!(all.iter().skip(1).all(|e| e.join), "{all:?}");
    assert_eq!(vim.text(), "ax\ncx\nex\n");
    assert_eq!(vim.position(), (0, 1), "the block's top-left");
    let again = edits(&mut vim, ".");
    assert_eq!(again.len(), 6, "`.` repeats the block change: {again:?}");
    assert_eq!(vim.text(), "ax\ncx\nex\n");
}

#[test]
fn gv_after_an_edit_above_selects_the_same_words() {
    // `<` and `>` need no tree: a Buffer of its own is enough for an
    // edit above the selection to shift them before `gv` reads them.
    let mut vim = on("one two\nthree four\n");
    typed(&mut vim, "wve<Esc>");
    typed(&mut vim, "Onew<Esc>");
    typed(&mut vim, "gvd");
    assert_eq!(
        vim.text(),
        "new\none \nthree four\n",
        "two, followed onto its new line"
    );
}

#[test]
fn gv_after_its_own_line_is_deleted_reselects_at_the_edits_first_line() {
    // A8: `<` and `>`, wholly taken by the `dd` that follows, relocate
    // to the edit's own first line rather than vanishing, the way
    // `[`, `]` and `.` do; `gv` still has something to reselect.
    let mut vim = on("one\ntwo\nthree\n");
    typed(&mut vim, "jve<Esc>");
    assert_eq!(typed(&mut vim, "0dd"), edit(4, 4, "", false));
    assert_eq!(vim.text(), "one\nthree\n", "two, taken whole");
    assert_eq!(typed(&mut vim, "gv"), Effect::Moved);
    typed(&mut vim, "d");
    assert_eq!(
        vim.text(),
        "one\nee\n",
        "the selection, now on three's own line"
    );
}

#[test]
fn dot_replaying_an_inserted_line_still_shifts_a_later_mark() {
    // `replay`'s own inner keys carry `self.repeat.replaying`, which
    // `changing` reads to skip `marked` there; the edits reach it
    // once, through the outer key that called `replay`, once it
    // returns them as one `Effect::Edits`. No case exercised this at
    // all: a mark past where `.` inserts a line must still move.
    //
    // The mark goes on "two", an interior line, not the text's own
    // last: a mark on the last line would still land there if `.`
    // shifted it twice (once inner, once outer) and a clamp brought
    // it back, hiding that fault behind a right-looking answer.
    let mut vim = on("one\ntwo\nthree\n");
    typed(&mut vim, "oNEW<Esc>");
    assert_eq!(
        vim.text(),
        "one\nNEW\ntwo\nthree\n",
        "one line opened below one"
    );
    assert_eq!(typed(&mut vim, "Gk0ma"), Effect::Moved);
    assert_eq!(vim.position(), (2, 0), "a, on two");
    typed(&mut vim, "gg.");
    assert_eq!(vim.text(), "one\nNEW\nNEW\ntwo\nthree\n");
    assert_eq!(typed(&mut vim, "'a"), Effect::Moved);
    assert_eq!(
        vim.position(),
        (3, 0),
        "a shifted once by the one line `.` added"
    );
}

#[test]
fn a_view_spans_the_selection_in_bytes() {
    let mut vim = on("abc\nde\nfghi\n");
    typed(&mut vim, "vj");
    let span = selected(&mut vim);
    assert_eq!(span, [(0, 5)], "through the newline");
    assert_eq!(vim.status(), "-- VISUAL --");
    typed(&mut vim, "o");
    assert_eq!(vim.position(), (0, 0), "`o` swaps the ends");
    assert_eq!(selected(&mut vim), span);
    typed(&mut vim, "V");
    assert_eq!(selected(&mut vim), [(0, 7)], "whole lines");
    assert_eq!(vim.status(), "-- VISUAL LINE --");
    typed(&mut vim, "<Esc>");
    assert_eq!(selected(&mut vim), Vec::new());
    typed(&mut vim, "gv");
    assert_eq!(vim.mode(), Mode::Visual(Visual::Line), "`gv` brings it back");
    typed(&mut vim, "<Esc>l<C-v>jj");
    assert_eq!(selected(&mut vim), [(1, 2), (5, 6), (8, 9)], "one per line");
    assert_eq!(vim.status(), "-- VISUAL BLOCK --");
    typed(&mut vim, "$");
    let span = selected(&mut vim);
    assert_eq!(span, [(1, 4), (5, 7), (8, 12)], "`$` reaches every end");
    let scrolled = spans(&mut vim, 1, SELECTION);
    assert_eq!(scrolled, [(8, 12)], "one row shows one line of it");
}

#[test]
fn hlsearch_marks_every_match_until_noh() {
    let mut vim = on("ab\nb\n");
    assert_eq!(typed(&mut vim, "/b<CR>"), Effect::Moved);
    assert_eq!(marked(&mut vim), Vec::new(), "off by default");
    // What `/self/vim/bin/set` and `.../nohlsearch` do to the engine;
    // the runs themselves are tests/ex.rs.
    assert_eq!(vim.set("hls"), Effect::Moved);
    assert_eq!(marked(&mut vim), [(1, 2), (3, 4)], "every match");
    assert_eq!(vim.nohlsearch(), Effect::Moved);
    assert_eq!(marked(&mut vim), Vec::new());
    assert_eq!(typed(&mut vim, "n"), Effect::Moved);
    assert_eq!(marked(&mut vim).len(), 2, "back with the next search");
    assert_eq!(vim.set("nohlsearch"), Effect::Moved);
    assert_eq!(marked(&mut vim), Vec::new());
    // The options are one argument, which splits itself.
    assert_eq!(vim.set("ic hls"), Effect::Moved);
    assert_eq!(typed(&mut vim, "/B<CR>"), Effect::Moved);
    assert_eq!(marked(&mut vim).len(), 2, "ignorecase");
    assert_eq!(vim.set("noic"), Effect::Moved);
    assert_eq!(typed(&mut vim, "/B<CR>"), Effect::Moved);
    assert_eq!(vim.status(), "E486: Pattern not found: B");
}

#[test]
fn messages_read_as_nvims() {
    let mut vim = on("  \nx\n");
    assert_eq!(typed(&mut vim, "*"), Effect::Moved, "a message is a change");
    assert_eq!(vim.status(), "E348: No string under cursor");
    assert_eq!(vim.set("foo"), Effect::Moved);
    assert_eq!(vim.status(), "E518: Unknown option: foo");
    // A pattern that does not parse changes nothing and says so; the
    // saying is the run's output cell, or the next key's own redraw.
    assert_eq!(vim.substitute((0, 0), "[/x/"), Effect::Nothing);
    assert_eq!(vim.status(), "E486: Pattern not found: [");
    assert_eq!(typed(&mut vim, "j"), Effect::Moved, "the message goes");
    assert_eq!(vim.status(), "");
}

#[test]
fn dot_repeats_a_visual_operator_over_the_same_size() {
    let mut vim = on("abcdef\n");
    assert_eq!(typed(&mut vim, "vld"), edit(0, 2, "", false));
    assert_eq!(
        typed(&mut vim, "."),
        Effect::Edits(vec![Edit {
            at: 0,
            len: 2,
            text: String::new(),
            join: false,
        }])
    );
    assert_eq!(vim.text(), "ef\n");
    assert_eq!(typed(&mut vim, "Vy"), Effect::Moved);
    let again = typed(&mut vim, ".");
    assert!(
        matches!(again, Effect::Edits(_)),
        "a yank is no change: {again:?}"
    );
    assert_eq!(vim.text(), "\n");
}

#[test]
fn registers_have_three_kinds_and_named_ones_append() {
    let mut vim = on("ab\ncd\n");
    typed(&mut vim, "\"ayw");
    assert_eq!(vim.register(), Some(&register("ab", Kind::Char)));
    typed(&mut vim, "j\"Ayw");
    assert_eq!(vim.register(), Some(&register("abcd", Kind::Char)), "appended");
    typed(&mut vim, "\"byy\"Byy");
    assert_eq!(vim.register(), Some(&register("cd\ncd", Kind::Line)));
    typed(&mut vim, "\"_dd");
    assert_eq!(vim.register(), Some(&register("cd\ncd", Kind::Line)), "kept");
    assert_eq!(vim.text(), "ab\n");
    typed(&mut vim, "<C-v>y");
    assert_eq!(vim.register(), Some(&register("a", Kind::Block)));
    assert_eq!(typed(&mut vim, "\"ap"), edit(1, 0, "abcd", false));
    assert_eq!(typed(&mut vim, "\"bp"), edit(7, 0, "cd\ncd\n", false));
    assert_eq!(vim.text(), "aabcdb\ncd\ncd\n");
}
