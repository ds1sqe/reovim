//! Vim alone, rung 2: what the corpus cannot see. The revisions an
//! operator makes, the cells a frame marks, the messages, `.` over
//! a selection, the registers' kinds.

mod common;

use common::{edit, edits, on, typed};
use reovim_client_vim::{Edit, Effect, Kind, Mode, Plugin, Register, Visual};

fn register(text: &str, kind: Kind) -> Register {
    Register { text: text.into(), kind }
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
    assert_eq!(typed(&mut vim, "J"), edit(0, 6, "xy baz", false));
    assert_eq!(typed(&mut vim, ">>"), edit(0, 6, "\txy baz", false));
    assert_eq!(typed(&mut vim, "wgUiw"), edit(1, 2, "XY", false));
    assert_eq!(typed(&mut vim, "~"), edit(1, 1, "x", false));
}

#[test]
fn many_places_change_as_one_revision() {
    let mut vim = on("a a\na\n");
    let all = edits(&mut vim, ":%s/a/b/g<CR>");
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
fn a_frame_marks_the_selection_in_cells() {
    let mut vim = on("abc\nde\nfghi\n");
    typed(&mut vim, "vj");
    let cells = vim.frame(80, 24).selection;
    assert_eq!(cells, vec![(0, 0, 4), (1, 0, 1)], "to the newline cell");
    assert_eq!(vim.frame(80, 24).status, "-- VISUAL --");
    typed(&mut vim, "o");
    assert_eq!(vim.position(), (0, 0), "`o` swaps the ends");
    assert_eq!(vim.frame(80, 24).selection, cells);
    typed(&mut vim, "V");
    assert_eq!(vim.frame(80, 24).selection, vec![(0, 0, 4), (1, 0, 3)]);
    assert_eq!(vim.frame(80, 24).status, "-- VISUAL LINE --");
    typed(&mut vim, "<Esc>");
    assert_eq!(vim.frame(80, 24).selection, Vec::new());
    typed(&mut vim, "gv");
    assert_eq!(vim.mode(), Mode::Visual(Visual::Line), "`gv` brings it back");
    typed(&mut vim, "<Esc>l<C-v>jj");
    assert_eq!(
        vim.frame(80, 24).selection,
        vec![(0, 1, 2), (1, 1, 2), (2, 1, 2)]
    );
    assert_eq!(vim.frame(80, 24).status, "-- VISUAL BLOCK --");
    typed(&mut vim, "$");
    let cells = vim.frame(80, 24).selection;
    assert_eq!(
        cells,
        vec![(0, 1, 4), (1, 1, 3), (2, 1, 5)],
        "`$` reaches every end"
    );
    assert_eq!(vim.frame(80, 2).selection, vec![(0, 1, 5)], "rows scrolled");
}

#[test]
fn hlsearch_marks_every_match_until_noh() {
    let mut vim = on("ab\nb\n");
    assert_eq!(typed(&mut vim, "/b<CR>"), Effect::Moved);
    assert_eq!(vim.frame(80, 24).marks, Vec::new(), "off by default");
    assert_eq!(typed(&mut vim, ":set hls<CR>"), Effect::Moved);
    assert_eq!(vim.frame(80, 24).marks, vec![(0, 1, 2), (1, 0, 1)]);
    assert_eq!(typed(&mut vim, ":noh<CR>"), Effect::Moved);
    assert_eq!(vim.frame(80, 24).marks, Vec::new());
    assert_eq!(typed(&mut vim, "n"), Effect::Moved);
    assert_eq!(vim.frame(80, 24).marks.len(), 2, "back with the next search");
    assert_eq!(typed(&mut vim, ":set nohlsearch<CR>"), Effect::Moved);
    assert_eq!(vim.frame(80, 24).marks, Vec::new());
    assert_eq!(typed(&mut vim, ":set ic hls<CR>/B<CR>"), Effect::Moved);
    assert_eq!(vim.frame(80, 24).marks.len(), 2, "ignorecase");
    assert_eq!(typed(&mut vim, ":set noic<CR>/B<CR>"), Effect::Moved);
    assert_eq!(vim.frame(80, 24).status, "E486: Pattern not found: B");
}

#[test]
fn messages_read_as_nvims() {
    let mut vim = on("  \nx\n");
    assert_eq!(typed(&mut vim, "*"), Effect::Moved, "a message is a change");
    assert_eq!(vim.frame(80, 24).status, "E348: No string under cursor");
    assert_eq!(typed(&mut vim, ":set foo<CR>"), Effect::Moved);
    assert_eq!(vim.frame(80, 24).status, "E518: Unknown option: foo");
    assert_eq!(typed(&mut vim, ":s/[/x/<CR>"), Effect::Moved);
    assert_eq!(vim.frame(80, 24).status, "E486: Pattern not found: [");
    assert_eq!(typed(&mut vim, ":frob<CR>"), Effect::Moved);
    assert_eq!(vim.frame(80, 24).status, "E492: Not an editor command: frob");
    assert_eq!(typed(&mut vim, "j"), Effect::Moved, "the message goes");
    assert_eq!(vim.frame(80, 24).status, "");
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
    assert_eq!(*vim.register(), register("ab", Kind::Char));
    typed(&mut vim, "j\"Ayw");
    assert_eq!(*vim.register(), register("abcd", Kind::Char), "appended");
    typed(&mut vim, "\"byy\"Byy");
    assert_eq!(*vim.register(), register("cd\ncd", Kind::Line));
    typed(&mut vim, "\"_dd");
    assert_eq!(*vim.register(), register("cd\ncd", Kind::Line), "kept");
    assert_eq!(vim.text(), "ab\n");
    typed(&mut vim, "<C-v>y");
    assert_eq!(*vim.register(), register("a", Kind::Block));
    assert_eq!(typed(&mut vim, "\"ap"), edit(1, 0, "abcd", false));
    assert_eq!(typed(&mut vim, "\"bp"), edit(7, 0, "cd\ncd\n", false));
    assert_eq!(vim.text(), "aabcdb\ncd\ncd\n");
}
