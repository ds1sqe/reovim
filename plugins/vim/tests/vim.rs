//! Vim alone, rung 1: keys in, text and frames out.

mod common;

use std::fmt::Write as _;

use common::{as_of, edit, on, tapped, typed};
use reovim_client_api::{BAR, BLOCK, MATCH, SELECTION, origin};
use reovim_plugin_vim::notation::keys;
use reovim_plugin_vim::{
    Edit, Effect, Ex, Filled, Key, Mode, Quit, Vim, Visual, Want,
};

/// The `:` line `keys` left for the Client to run: the range it read
/// off the front, and the rest of it as it was typed.
fn ex(from: u64, to: u64, line: &str) -> Effect {
    Effect::Line(Ex {
        ranged: from != 0 || to != 0,
        from,
        to,
        line: line.to_string(),
    })
}

#[test]
fn insert_edits_join_and_esc_steps_back() {
    let mut vim = on("");
    assert_eq!(vim.key(tapped(Key::Char('i'))), Effect::Moved);
    assert_eq!(vim.mode(), Mode::Insert);
    assert_eq!(vim.key(tapped(Key::Char('a'))), edit(0, 0, "a", false));
    assert_eq!(vim.key(tapped(Key::Char('b'))), edit(1, 0, "b", true));
    // The text had no final `\n`; the Enter ends its line and opens
    // the next, so the cursor's line is one the text holds.
    assert_eq!(vim.key(tapped(Key::Enter)), edit(2, 0, "\n\n", true));
    assert_eq!(vim.key(tapped(Key::Char('c'))), edit(3, 0, "c", true));
    assert_eq!(vim.position(), (1, 1));
    assert_eq!(vim.key(tapped(Key::Backspace)), edit(3, 1, "", true));
    assert_eq!(vim.key(tapped(Key::Backspace)), edit(2, 1, "", true), "across");
    assert_eq!(vim.text(), "ab\n", "one line, ended");
    let arrow = vim.key(tapped(Key::Left));
    assert_eq!(arrow, Effect::Moved, "an arrow ends the step");
    assert_eq!(vim.key(tapped(Key::Char('é'))), edit(1, 0, "é", false));
    assert_eq!(vim.key(tapped(Key::Esc)), Effect::Moved);
    assert_eq!(vim.mode(), Mode::Normal);
    assert_eq!(vim.position(), (0, 1), "back onto the char inserted");
    assert_eq!(vim.key(tapped(Key::Esc)), Effect::Nothing);
}

#[test]
fn the_entry_keys_put_the_cursor_where_vim_does() {
    let mut vim = on("  ab\ncd\n");
    assert_eq!(typed(&mut vim, "a1<Esc>"), Effect::Moved);
    assert_eq!(vim.text(), " 1 ab\ncd\n", "after the cursor's char");
    assert_eq!(vim.position(), (0, 1));
    assert_eq!(typed(&mut vim, "A2<Esc>"), Effect::Moved);
    assert_eq!(vim.text(), " 1 ab2\ncd\n");
    assert_eq!(typed(&mut vim, "I3<Esc>"), Effect::Moved);
    assert_eq!(vim.text(), " 31 ab2\ncd\n", "at the first non-blank");
    assert_eq!(typed(&mut vim, "o4<Esc>"), Effect::Moved);
    assert_eq!(vim.text(), " 31 ab2\n 4\ncd\n", "below, indented");
    assert_eq!(vim.position(), (1, 1));
    assert_eq!(typed(&mut vim, "O5<Esc>"), Effect::Moved);
    assert_eq!(vim.text(), " 31 ab2\n 5\n 4\ncd\n");
    assert_eq!(vim.position(), (1, 1));
    assert_eq!(typed(&mut vim, "o<Esc>"), edit(11, 1, "", true), "dropped");
    assert_eq!(vim.text(), " 31 ab2\n 5\n\n 4\ncd\n");
    assert_eq!(typed(&mut vim, "kA<CR>x<CR><CR>y<Esc>"), Effect::Moved);
    assert_eq!(vim.text(), " 31 ab2\n 5\n x\n\n y\n\n 4\ncd\n");
    let mut empty = on("\n");
    assert_eq!(typed(&mut empty, "ax<Esc>"), Effect::Moved);
    assert_eq!(empty.text(), "x\n", "an empty line appends in place");
}

#[test]
fn an_unbound_alt_is_esc_then_the_char() {
    // Normal: the count goes with the Esc, `j` moves a line down and
    // `x` is normal mode's own.
    let mut vim = on("abc\ndef\nghi\n");
    typed(&mut vim, "2<M-j>x");
    assert_eq!(vim.text(), "abc\nef\nghi\n");
    assert_eq!(vim.position(), (1, 0));
    assert_eq!(vim.showcmd(), "", "the count went with the Esc");

    // An operator waiting is dropped by the Esc, not applied to `j`.
    let mut vim = on("abc\ndef\nghi\n");
    typed(&mut vim, "d<M-j>x");
    assert_eq!(vim.text(), "abc\nef\nghi\n");

    // Insert: the Esc ends it and the char is normal mode's.
    let mut vim = on("abc\ndef\nghi\n");
    typed(&mut vim, "iX<M-j>x");
    assert_eq!(vim.text(), "Xabc\nef\nghi\n");
    assert_eq!(vim.mode(), Mode::Normal);

    // Visual: the Esc leaves it, so `x` takes one char and not the
    // selection.
    let mut vim = on("abc\ndef\nghi\n");
    typed(&mut vim, "vl<M-j>x");
    assert_eq!(vim.text(), "abc\ndf\nghi\n");
    assert_eq!(vim.mode(), Mode::Normal);

    // The `:` line and the search line: the Esc drops what was typed.
    let mut vim = on("abc\ndef\nghi\n");
    typed(&mut vim, ":2<M-j>x");
    assert_eq!(vim.text(), "abc\nef\nghi\n");
    assert_eq!(vim.mode(), Mode::Normal);
    let mut vim = on("abc\ndef\nghi\n");
    typed(&mut vim, "/e<M-j>x");
    assert_eq!(vim.text(), "abc\nef\nghi\n");
    assert_eq!(vim.mode(), Mode::Normal);
}

#[test]
fn a_bound_prefix_lets_go_of_what_it_held() {
    // `dx` bound and `dd`: the two keys are let go and `dd` deletes a
    // line, as it always did.
    let mut vim = on("abc\ndef\n");
    vim.bind("dx", "write");
    typed(&mut vim, "dd");
    assert_eq!(vim.text(), "def\n");

    // `xy` bound and `xp`: two chars still swap.
    let mut vim = on("abc\n");
    vim.bind("xy", "write");
    typed(&mut vim, "xp");
    assert_eq!(vim.text(), "bac\n");

    // `,w` and `,wab` bound: `,wax` runs the longest head that is a
    // binding, and `a` and `x` after it are typed keys.
    let mut vim = on("abc\n");
    vim.bind(",w", "write");
    vim.bind(",wab", "quit");
    typed(&mut vim, ",wax");
    assert_eq!(vim.text(), "axbc\n");
    assert_eq!(vim.mode(), Mode::Insert);

    // `\w` bound and `\<M-j>x`: no head is a binding, so both keys
    // are let go, and the Alt let go is `<Esc>` then `j`.
    let mut vim = on("abc\ndef\nghi\n");
    vim.bind("\\w", "write");
    typed(&mut vim, "\\<M-j>x");
    assert_eq!(vim.text(), "abc\nef\nghi\n");
    assert_eq!(vim.position(), (1, 0));
}

#[test]
fn normal_motions_stay_on_chars_and_take_counts() {
    let mut vim = on("abc\n\nlonger line\n  x\n");
    assert_eq!(typed(&mut vim, "l"), Effect::Moved);
    assert_eq!(typed(&mut vim, "5l"), Effect::Moved);
    assert_eq!(vim.position(), (0, 2), "not past the last char");
    assert_eq!(typed(&mut vim, "l"), Effect::Nothing);
    assert_eq!(typed(&mut vim, "j"), Effect::Moved);
    assert_eq!(vim.position(), (1, 0), "an empty line");
    assert_eq!(typed(&mut vim, "j$"), Effect::Moved);
    assert_eq!(vim.position(), (2, 10));
    assert_eq!(typed(&mut vim, "k"), Effect::Moved);
    assert_eq!(vim.position(), (1, 0));
    assert_eq!(typed(&mut vim, "2j"), Effect::Moved);
    assert_eq!(vim.position(), (3, 2), "still wanting the end");
    assert_eq!(typed(&mut vim, "hkkjj"), Effect::Moved);
    assert_eq!(vim.position(), (3, 1), "the column wanted survives");
    assert_eq!(typed(&mut vim, "0"), Effect::Moved);
    assert_eq!(vim.position(), (3, 0));
    assert_eq!(typed(&mut vim, "^"), Effect::Moved);
    assert_eq!(vim.position(), (3, 2));
    assert_eq!(typed(&mut vim, "10h"), Effect::Moved);
    assert_eq!(vim.position(), (3, 0));
    assert_eq!(typed(&mut vim, "gg"), Effect::Moved);
    assert_eq!(vim.position(), (0, 0));
    assert_eq!(typed(&mut vim, "3k2$"), Effect::Moved);
    assert_eq!(vim.position(), (1, 0), "2$ is the end of the next line");
    assert_eq!(typed(&mut vim, "9<Esc>l"), Effect::Nothing, "Esc drops 9");
}

#[test]
fn loaded_replaces_and_clamps() {
    let mut vim = on("hello");
    assert_eq!(typed(&mut vim, "$"), Effect::Moved);
    assert!(!vim.loaded(b"hello"), "the same text");
    assert!(vim.loaded(b"hi"));
    assert_eq!(vim.position(), (0, 1), "clamped onto the last char");
    assert!(vim.loaded("aé".as_bytes()));
    assert_eq!(vim.position(), (0, 1), "clamped to a char boundary");
    assert!(vim.loaded(b"\xff"));
    assert_eq!(vim.text(), "\u{fffd}", "lossy");
    assert_eq!(typed(&mut vim, "A"), Effect::Moved);
    assert!(vim.loaded(b"xy"));
    assert_eq!(vim.position(), (0, 2), "insert may sit past the end");
}

#[test]
fn loaded_moves_marks_by_its_own_real_diff() {
    // `loaded`'s synthetic edit must come from `minimal_edit` reading
    // the two texts themselves, not a hand-built `Edit`: a line
    // inserted before the mark's own shifts it down by one.
    let mut vim = on("one\ntwo\nthree\n");
    assert_eq!(typed(&mut vim, "Gma"), Effect::Moved);
    assert!(vim.loaded(b"one\ntwo\nTWO\nthree\n"));
    assert_eq!(typed(&mut vim, "'a"), Effect::Moved);
    assert_eq!(vim.position(), (3, 0), "the mark moved down with its line");
}

#[test]
fn loaded_diffs_by_chars_not_bytes_across_multibyte_ones() {
    // "é" and "ç" share their own first UTF-8 byte: a byte-based
    // prefix would stop mid-char, and slicing there panics.
    // `minimal_edit` must walk `.chars()`, never raw bytes.
    let mut vim = on("é\n");
    assert!(vim.loaded("ç\n".as_bytes()));
    assert_eq!(vim.text(), "ç\n");
}

#[test]
fn fixed_eol_reaches_marked_so_dd_of_that_line_takes_its_mark() {
    // Without `fixed_eol`'s own edit reaching `marked`, the index
    // keeps thinking the text has no final `\n`, so `dd` of the
    // marked line (now taking that `\n` with it) reads the line as
    // shorter than it is and its mark survives where nvim's does not.
    let mut vim = on("a\nb");
    assert_eq!(typed(&mut vim, "jma"), Effect::Moved);
    assert!(vim.fixed_eol().is_some(), "no final newline to add one to");
    assert_eq!(typed(&mut vim, "dd"), edit(2, 2, "", false));
    assert_eq!(typed(&mut vim, "'a"), Effect::Moved, "a message is a change");
    assert_eq!(vim.status(), "E20: Mark not set");
}

#[test]
fn a_view_keeps_the_cursor_in_the_region() {
    let mut vim = on("1\n2\n3\n4\n5 is long");
    typed(&mut vim, "4j$");
    let view = vim.view(2);
    assert_eq!((view.top, view.cursor), (6, 16), "scrolled to the last line");
    assert_eq!(view.status, "");
    typed(&mut vim, "4k");
    let view = vim.view(2);
    assert_eq!((view.top, view.cursor), (0, 0), "and back to the first");
    typed(&mut vim, "2j");
    let view = vim.view(2);
    assert_eq!((view.top, view.cursor), (2, 4), "one line at a time");
    assert_eq!(vim.view(1).top, 4, "one row shows the cursor's line");
    typed(&mut vim, "i");
    let view = vim.view(2);
    assert_eq!(view.status, "-- INSERT --");
    assert_eq!(view.shape, BAR, "insert stands between chars");
    typed(&mut vim, "<Esc>");
    assert_eq!(vim.view(2).shape, BLOCK, "every other mode is on one");
}

#[test]
fn words_move_as_vim_moves_them() {
    let mut vim = on("foo.bar  baz\n\n  qux\n");
    assert_eq!(typed(&mut vim, "w"), Effect::Moved);
    assert_eq!(vim.position(), (0, 3), "punctuation is a word");
    assert_eq!(typed(&mut vim, "w"), Effect::Moved);
    assert_eq!(vim.position(), (0, 4));
    assert_eq!(typed(&mut vim, "2w"), Effect::Moved);
    assert_eq!(vim.position(), (1, 0), "an empty line is a word");
    assert_eq!(typed(&mut vim, "w"), Effect::Moved);
    assert_eq!(vim.position(), (2, 2));
    assert_eq!(typed(&mut vim, "w"), Effect::Moved);
    assert_eq!(vim.position(), (2, 4), "the last char at the end");
    assert_eq!(typed(&mut vim, "w"), Effect::Nothing);
    assert_eq!(typed(&mut vim, "b"), Effect::Moved);
    assert_eq!(vim.position(), (2, 2));
    assert_eq!(typed(&mut vim, "b"), Effect::Moved);
    assert_eq!(vim.position(), (1, 0));
    assert_eq!(typed(&mut vim, "b"), Effect::Moved);
    assert_eq!(vim.position(), (0, 9));
    assert_eq!(typed(&mut vim, "3b"), Effect::Moved);
    assert_eq!(vim.position(), (0, 0));
    assert_eq!(typed(&mut vim, "b"), Effect::Nothing);
    assert_eq!(typed(&mut vim, "e"), Effect::Moved);
    assert_eq!(vim.position(), (0, 2));
    assert_eq!(typed(&mut vim, "e"), Effect::Moved);
    assert_eq!(vim.position(), (0, 3), "the end of `.`");
    assert_eq!(typed(&mut vim, "2e"), Effect::Moved);
    assert_eq!(vim.position(), (0, 11));
    assert_eq!(typed(&mut vim, "e"), Effect::Moved);
    assert_eq!(vim.position(), (2, 4), "past the empty line");
}

#[test]
fn lines_are_reached_by_number_and_deleted_with_their_newline() {
    let mut vim = on("one\n  two\nthree\n");
    assert_eq!(typed(&mut vim, "G"), Effect::Moved);
    assert_eq!(vim.position(), (2, 0));
    assert_eq!(typed(&mut vim, "gg"), Effect::Moved);
    assert_eq!(vim.position(), (0, 0));
    assert_eq!(typed(&mut vim, "9G"), Effect::Moved);
    assert_eq!(vim.position(), (2, 0), "clamped");
    assert_eq!(typed(&mut vim, "$2gg"), Effect::Moved);
    assert_eq!(vim.position(), (1, 4), "the column is kept");
    assert_eq!(typed(&mut vim, "gx"), Effect::Nothing, "g then not g");
    assert_eq!(vim.text(), "one\n  two\nthree\n");

    assert_eq!(typed(&mut vim, "dd"), edit(4, 6, "", false));
    assert_eq!(vim.text(), "one\nthree\n");
    assert_eq!(vim.position(), (1, 4), "`$` still wants the end");
    assert_eq!(typed(&mut vim, "dd"), edit(4, 6, "", false), "the last");
    assert_eq!(vim.text(), "one\n");
    assert_eq!(vim.position(), (0, 2));
    assert_eq!(typed(&mut vim, "dd"), edit(0, 4, "", false));
    assert_eq!(vim.text(), "");
    assert_eq!(typed(&mut vim, "dd"), Effect::Nothing);
    let mut vim = on("one\ntwo");
    assert_eq!(typed(&mut vim, "jdd"), edit(3, 4, "", false), "no newline");
    assert_eq!(vim.text(), "one");

    let mut vim = on("a\nb\nc\nd\n");
    assert_eq!(typed(&mut vim, "j3dd"), edit(2, 6, "", false), "past the end");
    assert_eq!(vim.text(), "a\n");
    assert_eq!(vim.position(), (0, 0));
    let mut vim = on("");
    assert_eq!(typed(&mut vim, "G"), Effect::Nothing);
    vim.begin(9);
    assert_eq!(vim.position(), (0, 0));
}

#[test]
fn chars_are_deleted_on_the_line_and_undo_is_a_key() {
    let mut vim = on("abcd\n\nxy\n");
    assert_eq!(typed(&mut vim, "x"), edit(0, 1, "", false));
    assert_eq!(vim.text(), "bcd\n\nxy\n");
    assert_eq!(typed(&mut vim, "$x"), edit(2, 1, "", false));
    assert_eq!(vim.position(), (0, 1), "back onto the last char");
    assert_eq!(typed(&mut vim, "5x"), edit(1, 1, "", false), "the line ends");
    assert_eq!(vim.text(), "b\n\nxy\n");
    assert_eq!(typed(&mut vim, "j"), Effect::Moved);
    assert_eq!(typed(&mut vim, "x"), Effect::Nothing, "an empty line");
    assert_eq!(typed(&mut vim, "X"), Effect::Nothing, "nothing before");
    assert_eq!(typed(&mut vim, "j$X"), edit(3, 1, "", false));
    assert_eq!(vim.text(), "b\n\ny\n");
    assert_eq!(vim.position(), (2, 0));
    assert_eq!(typed(&mut vim, "D"), edit(3, 1, "", false));
    assert_eq!(vim.text(), "b\n\n\n");
    assert_eq!(typed(&mut vim, "u"), Effect::Undo);
    assert_eq!(typed(&mut vim, "<C-r>"), Effect::Redo);
    let mut vim = on("ab cd");
    assert_eq!(typed(&mut vim, "wD"), edit(3, 2, "", false));
    assert_eq!(vim.text(), "ab ");
    assert_eq!(vim.position(), (0, 2));
}

#[test]
fn a_last_line_without_a_newline_gets_one_before_a_write() {
    let mut vim = on("abc");
    let eol = Edit { at: 3, len: 0, text: "\n".into(), join: false };
    assert_eq!(vim.fixed_eol(), Some(eol));
    assert_eq!(vim.fixed_eol(), None);
    assert_eq!(vim.text(), "abc\n");
    assert_eq!(typed(&mut vim, "j"), Effect::Nothing);
    let mut empty = on("");
    assert_eq!(empty.fixed_eol(), None);
    assert_eq!(empty.text(), "", "and an empty text needs none");
}

#[test]
fn the_command_line_hands_its_line_on_with_the_range_it_read() {
    let mut vim = on("abc\ndef\n");
    assert_eq!(typed(&mut vim, ":"), Effect::Moved);
    assert_eq!(vim.mode(), Mode::Command);
    assert_eq!(typed(&mut vim, "wq"), Effect::Moved);
    assert_eq!(vim.status(), ":wq");
    assert_eq!(typed(&mut vim, "<BS><BS>"), Effect::Moved);
    assert_eq!(vim.status(), ":");
    assert_eq!(typed(&mut vim, "<BS>"), Effect::Moved, "leaves");
    assert_eq!(vim.mode(), Mode::Normal);
    assert_eq!(typed(&mut vim, ":q<Esc>"), Effect::Moved);
    assert_eq!(vim.mode(), Mode::Normal);
    assert_eq!(typed(&mut vim, ":<CR>"), Effect::Moved, "an empty line");

    // The engine reads the range and nothing else: what the rest names
    // is the Client's to resolve over the tree, and the argument is
    // one argument, spaces and all, by Vim's own rule.
    assert_eq!(typed(&mut vim, ":w<CR>"), ex(0, 0, "w"));
    assert_eq!(typed(&mut vim, ":w!<CR>"), ex(0, 0, "w!"));
    assert_eq!(typed(&mut vim, ":w  a b <CR>"), ex(0, 0, "w  a b"));
    assert_eq!(typed(&mut vim, ":w! a b<CR>"), ex(0, 0, "w! a b"));
    assert_eq!(typed(&mut vim, ": x <CR>"), ex(0, 0, "x"));
    assert_eq!(typed(&mut vim, ":%s/a/b/g<CR>"), ex(0, 1, "s/a/b/g"));
    assert_eq!(typed(&mut vim, ":2s/a/b/<CR>"), ex(1, 1, "s/a/b/"));
    assert_eq!(typed(&mut vim, ":1,$d<CR>"), ex(0, 1, "d"));
    // A name Vim has none of goes the same way, whole and as it was
    // typed; E492 comes back from the tree, where there is one to
    // answer it (tests/explore.rs).
    assert_eq!(typed(&mut vim, ":xyz<CR>"), ex(0, 0, "xyz"));
    assert_eq!(vim.status(), "", "nothing is said without a tree");

    // The two quits that are keys and not lines stay the engine's.
    assert_eq!(typed(&mut vim, "ZZ"), Effect::Quit(Quit::Write));
    assert_eq!(typed(&mut vim, "ZQ"), Effect::Quit(Quit::Force));
    assert_eq!(typed(&mut vim, "Zx"), Effect::Nothing);
    assert_eq!(typed(&mut vim, "<C-s>"), Effect::Nothing, "no longer a key");
    assert_eq!(typed(&mut vim, "<C-q>"), Effect::Nothing);
}

#[test]
fn the_clipboard_registers_wait_for_their_rd() {
    let mut vim = on("one\ntwo\n");
    assert_eq!(typed(&mut vim, "\"+yy"), Effect::Moved);
    assert_eq!(vim.clipped(), Some("one\n".to_string()), "linewise");
    assert_eq!(vim.clipped(), None, "taken once");
    assert_eq!(typed(&mut vim, "\"*yiw"), Effect::Moved);
    assert_eq!(vim.clipped(), Some("one".to_string()));

    assert_eq!(typed(&mut vim, "j\"+p"), Effect::Clipboard, "wants the text");
    assert_eq!(vim.pasted(Some("x\n".to_string())), edit(8, 0, "x\n", false));
    assert_eq!(vim.text(), "one\ntwo\nx\n");
    assert_eq!(typed(&mut vim, "\"+P"), Effect::Clipboard);
    assert_eq!(vim.pasted(None), Effect::Moved, "no clipboard");
    assert!(vim.status().starts_with("E353"));
    assert_eq!(typed(&mut vim, "\"+p"), Effect::Clipboard);
    assert_eq!(vim.pasted(Some("ab".to_string())), edit(9, 0, "ab", false));
    assert_eq!(vim.text(), "one\ntwo\nxab\n", "charwise after the cursor");
    let again = Effect::Edits(vec![Edit {
        at: 11,
        len: 0,
        text: "ab".into(),
        join: false,
    }]);
    assert_eq!(typed(&mut vim, "."), again, "repeats");
}

#[test]
fn fed_keys_do_not_record_and_repeat_preserves_the_last_change() {
    let mut vim = on("abcdefgh\n");
    typed(&mut vim, "qb");
    vim.key(as_of(Key::Char('x'), origin::FED));
    typed(&mut vim, "q");
    assert_eq!(vim.stored('b'), "");
    typed(&mut vim, "x");
    vim.key(as_of(Key::Char('l'), origin::REPEAT));
    typed(&mut vim, ".");
    assert_eq!(vim.text(), "cefgh\n");
}

#[test]
fn a_motion_before_the_first_fed_edit_does_not_join_prior_typing() {
    let mut vim = on("abcdef\nghijkl\n");
    typed(&mut vim, "x");
    vim.key(as_of(Key::Char('j'), origin::FED));
    let fed = |vim: &mut Vim| vim.key(as_of(Key::Char('x'), origin::FED));
    assert_eq!(fed(&mut vim), edit(6, 1, "", false));
    assert_eq!(fed(&mut vim), edit(6, 1, "", true));
    typed(&mut vim, "'[");
    assert_eq!(vim.position(), (1, 0));
}

#[test]
fn errors_ring_but_permitted_stationary_commands_do_not() {
    for input in ["h", "j", "fz", "%", "'a"] {
        let mut vim = on("abc\n");
        typed(&mut vim, input);
        assert!(vim.rung(), "{input}");
        assert!(!vim.rung(), "the bell is taken");
    }
    for (text, input) in [
        ("abc\n", "0"),
        ("abc\n", "gg"),
        ("abc\n", "i<Left>"),
        ("\n", "x"),
        ("abc\n", "X"),
    ] {
        let mut vim = on(text);
        typed(&mut vim, input);
        assert!(!vim.rung(), "{input}");
    }
}

#[test]
fn a_fed_line_does_not_replace_the_last_typed_ex() {
    let mut vim = on("abc\n");
    typed(&mut vim, ":set ic<CR>");
    for key in reovim_plugin_vim::notation::keys(":set noic<CR>").unwrap() {
        vim.key(as_of(key, origin::FED));
    }
    let effect = typed(&mut vim, "@:");
    assert_eq!(
        effect,
        Effect::Fed(
            reovim_plugin_vim::notation::keys(":set ic<CR>").unwrap(),
            false
        )
    );
}

#[test]
fn huge_counts_on_empty_registers_finish_and_nonempty_runs_are_bounded() {
    let mut vim = on("x\n");
    assert_eq!(
        typed(&mut vim, "999999999999999999999999999@a"),
        Effect::Fed(vec![], false)
    );
    typed(&mut vim, "\"ayiw");
    let Effect::Fed(keys, false) = typed(&mut vim, "200000@a") else {
        panic!("a register run must return replay keys");
    };
    assert_eq!(keys, vec![Key::Char('x'); 100_001]);
}

#[test]
fn h_m_l_read_the_window_and_clamp_counts() {
    let text: String = (1..=20).fold(String::new(), |mut text, n| {
        writeln!(text, "{n}").unwrap();
        text
    });
    let mut vim = on(&text);
    let _ = vim.view(5);
    for (keys, line) in
        [("99H", 4), ("99L", 0), ("3M", 2), ("GH", 15), ("M", 17), ("L", 19)]
    {
        typed(&mut vim, keys);
        assert_eq!(vim.position().0, line, "{keys}");
    }
    let mut vim = on("a\nb\n");
    let _ = vim.view(0);
    typed(&mut vim, "999999999999999999999999L");
    assert_eq!(vim.position().0, 0, "zero-sized view is one row");
}

#[test]
fn a_window_motion_pushes_a_jump() {
    let mut vim = on("one\ntwo\nthree\nfour\nfive\n");
    let _ = vim.view(5);
    typed(&mut vim, "jL");
    assert_eq!(vim.position().0, 4);
    typed(&mut vim, "<C-o>");
    assert_eq!(vim.position().0, 1);
}

#[test]
fn window_operator_counts_saturate_before_clamping() {
    let mut vim = on("one\ntwo\nthree\nfour\nfive\n");
    let _ = vim.view(5);
    typed(&mut vim, "2d999999999999999999999999H");
    assert_eq!(vim.position(), (0, 0));
    assert!(!vim.text().contains("one"));
    assert!(!vim.text().contains("five"));
}

#[test]
fn waiting_for_a_key_reads_digits_as_the_key() {
    let mut vim = on("a1b2c\n");
    typed(&mut vim, "f1");
    assert_eq!(vim.position(), (0, 1));
    typed(&mut vim, "0t2");
    assert_eq!(vim.position(), (0, 2));
    let mut vim = on("abc\n");
    typed(&mut vim, "vlr3");
    assert_eq!(vim.text(), "33c\n");
}

#[test]
fn ordinary_counts_and_word_objects_remain_unchanged() {
    let mut vim = on("one two three four\n");
    typed(&mut vim, "2w");
    assert_eq!(vim.position(), (0, 8));
    typed(&mut vim, "diw");
    assert_eq!(vim.text(), "one two  four\n");
    let mut vim = on("a.b c-d\n");
    typed(&mut vim, "diW");
    assert_eq!(vim.text(), " c-d\n");
}

#[test]
fn a_maximum_height_after_scrolling_does_not_overflow() {
    let mut vim = on("one\ntwo\nthree\n");
    typed(&mut vim, "G");
    let _ = vim.view(1);
    let view = vim.view(usize::MAX);
    assert_eq!(view.cursor, 8);
    typed(&mut vim, "H");
    assert_eq!(vim.position(), (2, 0));
}

#[test]
fn maximum_height_is_safe_with_visual_selection_after_scrolling() {
    for (keys, end) in [("v", 9), ("V", 14), ("<C-v>", 9)] {
        let mut vim = on("one\ntwo\nthree\n");
        typed(&mut vim, "G");
        let _ = vim.view(1);
        typed(&mut vim, keys);
        let view = vim.view(usize::MAX);
        assert_eq!(view.cursor, 8);
        assert!(view.spans.iter().any(|span| span.from == 8
            && span.to == end
            && span.style == reovim_client_api::SELECTION));
    }
}

#[test]
fn dot_is_keys_for_the_queue() {
    let mut vim = on("abcdef\n");
    let dot = vim.key(tapped(Key::Char('.')));
    assert_eq!(dot, Effect::Nothing, "no last change");
    assert_eq!(typed(&mut vim, "x"), edit(0, 1, "", false));
    // The engine replays nothing itself: `.` answers the keys, and
    // the Plugin's queue runs them as a repeat's.
    let dot = vim.key(tapped(Key::Char('.')));
    assert_eq!(dot, Effect::Fed(vec![Key::Char('x')], true));
    assert_eq!(vim.text(), "bcdef\n", "the engine ran none of them");
    vim.key(tapped(Key::Char('3')));
    assert_eq!(
        vim.key(tapped(Key::Char('.'))),
        Effect::Fed(vec![Key::Char('3'), Key::Char('x')], true),
        "a count typed before `.` leads the keys it answers"
    );
}

#[test]
fn a_repeats_first_edit_pushes_the_undo_cursor() {
    let mut vim = on("abc\nabc\n");
    typed(&mut vim, "A!<Esc>j0.");
    assert_eq!(vim.text(), "abc!\nabc!\n");
    assert_eq!(typed(&mut vim, "u"), Effect::Undo);
    // What the Plugin reads back once the history has walked.
    vim.loaded(b"abc!\nabc\n");
    assert_eq!(
        vim.position(),
        (1, 2),
        "the undo cursor is where the repeat's first edit began"
    );
}

/// What the door asks for over `pattern`, a list begun at `from`.
fn want(vim: &Vim, pattern: &str, from: u64) -> Effect {
    Effect::Search(Want {
        pattern: pattern.into(),
        ignorecase: false,
        smartcase: false,
        from,
        epoch: vim.epoch(),
    })
}

#[test]
fn a_search_asks_and_lands_when_answered() {
    let mut vim = on("ab\ncd\n");
    let epoch = vim.epoch();
    // Nothing of the text is scanned here: the key asks, and the
    // cursor stands until the entries come back.
    assert_eq!(typed(&mut vim, "/cd<CR>"), want(&vim, "cd", 0));
    assert_eq!(vim.position(), (0, 0), "nothing moved while it waits");
    // An answer for a list that has gone is nobody's.
    assert_eq!(
        vim.matched(epoch + 9, &[(0, 2)], Filled::Whole),
        Effect::Nothing
    );
    assert_eq!(vim.position(), (0, 0), "and still nothing moved");
    assert_eq!(vim.matched(epoch, &[(3, 2)], Filled::Whole), Effect::Moved);
    assert_eq!(vim.position(), (1, 0));
}

#[test]
fn an_operator_waits_with_its_search() {
    let mut vim = on("ab\n");
    let epoch = vim.epoch();
    assert_eq!(typed(&mut vim, "d/b<CR>"), want(&vim, "b", 0));
    assert_eq!(vim.showcmd(), "d", "the operator stands through the wait");
    assert_eq!(
        vim.matched(epoch, &[(1, 1)], Filled::Whole),
        edit(0, 1, "", false)
    );
    assert_eq!(vim.text(), "b\n");
    assert_eq!(vim.showcmd(), "");
    assert_eq!(
        vim.key(tapped(Key::Char('.'))),
        Effect::Fed(keys("d/b<CR>").unwrap(), true),
        "the record stood through the wait, so `.` is the four keys"
    );
}

#[test]
fn a_list_not_whole_answers_from_its_run_and_asks_for_the_rest() {
    let mut vim = on("ab\nxb\ncb\n");
    typed(&mut vim, "j");
    let epoch = vim.epoch();
    assert_eq!(typed(&mut vim, "/b<CR>"), want(&vim, "b", 3));
    // An entry after the cursor in the cursor's own run is final as
    // read, though the fill has more to give: the fill goes in order,
    // so it found everything between the two.
    assert_eq!(
        vim.matched(epoch, &[(4, 1), (7, 1)], Filled::More),
        Effect::Moved
    );
    assert_eq!(vim.position(), (1, 1));
    assert_eq!(typed(&mut vim, "n"), Effect::Moved, "answered from the run");
    assert_eq!(vim.position(), (2, 1));
    // The wrap's own run, which nobody waited for.
    assert_eq!(vim.matched(epoch, &[(1, 1)], Filled::More), Effect::Nothing);
    // An entry past the cursor's own run is not final until the list
    // is whole, though it is already read: it asks again, and for the
    // list it began, not for one from the cursor.
    typed(&mut vim, "ggl");
    assert_eq!(typed(&mut vim, "n"), want(&vim, "b", 3));
    assert_eq!(vim.matched(epoch, &[], Filled::Whole), Effect::Moved);
    assert_eq!(vim.position(), (1, 1));
}

#[test]
fn a_match_on_a_lines_end_is_passed_by_a_cursor_beside_it() {
    let mut vim = on("ab\ncd\n");
    typed(&mut vim, "G$");
    assert_eq!(vim.position(), (1, 1));
    let epoch = vim.epoch();
    assert_eq!(typed(&mut vim, "/$<CR>"), want(&vim, "$", 4));
    assert_eq!(
        vim.matched(epoch, &[(2, 0), (5, 0)], Filled::Whole),
        Effect::Moved
    );
    assert_eq!(
        vim.position(),
        (0, 1),
        "the entry beside the cursor is passed; the landing is the last char"
    );
}

#[test]
fn a_word_takes_the_operator_and_the_record() {
    let mut vim = on("ab cd ef\n");
    let epoch = vim.epoch();
    assert_eq!(typed(&mut vim, "d/\\(<CR>"), want(&vim, "\\(", 0));
    vim.matched(epoch, &[], Filled::Refused);
    assert_eq!(vim.status(), "E486: Pattern not found: \\(");
    assert!(vim.rung(), "the word rings, so a run ends at it");
    assert_eq!(vim.showcmd(), "", "the operator went with the motion");
    assert_eq!(typed(&mut vim, "w"), Effect::Moved);
    assert_eq!(vim.text(), "ab cd ef\n", "the text is whole");
    assert_eq!(vim.position(), (0, 3));

    let mut vim = on("ab cd ef\n");
    let epoch = vim.epoch();
    assert_eq!(typed(&mut vim, "d/zzz<CR>"), want(&vim, "zzz", 0));
    vim.matched(epoch, &[], Filled::Whole);
    assert_eq!(vim.status(), "E486: Pattern not found: zzz");
    assert_eq!(typed(&mut vim, "w"), Effect::Moved);
    assert_eq!(vim.text(), "ab cd ef\n");

    let mut vim = on("abcd\n");
    let epoch = vim.epoch();
    assert_eq!(typed(&mut vim, "/\\(<CR>"), want(&vim, "\\(", 0));
    vim.matched(epoch, &[], Filled::Refused);
    typed(&mut vim, "x.");
    assert_eq!(vim.text(), "cd\n", "the record the word cleared is the `x`'s");
}

#[test]
fn entries_before_a_spent_offset_are_landed_on() {
    let text = format!("x\nb\n{}\n", "a".repeat(30));
    let mut vim = on(&text);
    let epoch = vim.epoch();
    assert_eq!(typed(&mut vim, "/\\(a*\\)*b<CR>"), want(&vim, "\\(a*\\)*b", 0));
    // What the slice found before it spent its budget comes first.
    assert_eq!(vim.matched(epoch, &[(2, 1)], Filled::More), Effect::Moved);
    assert_eq!(vim.position(), (1, 0));
    assert_eq!(vim.matched(epoch, &[], Filled::Spent), Effect::Nothing);
    assert_eq!(
        vim.status(),
        "/\\(a*\\)*b  [?/1+]",
        "the landing said no word, and its row holds what was found"
    );
    assert_eq!(typed(&mut vim, "n"), Effect::Moved, "the message is a change");
    assert!(vim.status().starts_with("E363"), "{}", vim.status());
}

#[test]
fn a_spent_list_nobody_waits_for_goes_stale() {
    let mut vim = on("abc\nabc\n");
    assert_eq!(vim.set("hls"), Effect::Moved);
    let epoch = vim.epoch();
    assert_eq!(typed(&mut vim, "/b<CR>"), want(&vim, "b", 0));
    assert_eq!(
        vim.matched(epoch, &[(1, 1), (5, 1)], Filled::Whole),
        Effect::Moved
    );
    assert!(vim.wants().is_none(), "a whole list wants nothing more");
    typed(&mut vim, "x");
    let asked = vim.wants().expect("the edit dropped the list");
    assert_eq!(vim.matched(asked.epoch, &[], Filled::Spent), Effect::Nothing);
    assert!(vim.wants().is_none(), "stale: no frame pays the offset again");
    typed(&mut vim, "x");
    assert!(vim.wants().is_none(), "nor after another edit");
}

#[test]
fn a_landing_where_the_cursor_stands_pushes_no_jump() {
    let mut vim = on("ab\ncd\n");
    typed(&mut vim, "G$");
    let epoch = vim.epoch();
    assert_eq!(typed(&mut vim, "/$<CR>"), want(&vim, "$", 4));
    assert_eq!(vim.matched(epoch, &[(5, 0)], Filled::Whole), Effect::Moved);
    assert_eq!(vim.position(), (1, 1), "the landing is where it stood");
    // `'` is set where a jump is pushed, and neither happened here:
    // it still answers the default place a Buffer opens at.
    typed(&mut vim, "``");
    assert_eq!(vim.position(), (0, 0));
}

#[test]
fn a_wait_runs_again_as_its_key_came() {
    // A macro's `<CR>`, let go by a failed prefix and run again by
    // `past`: the landing's edit joins the run it came from.
    let mut vim = on("ab\nxb\n");
    vim.key(as_of(Key::Char('x'), origin::FED));
    for key in keys("d/b").unwrap() {
        vim.key(as_of(key, origin::FED));
    }
    let epoch = vim.epoch();
    let asked = vim.key(as_of(Key::Enter, origin::RELEASED));
    assert_eq!(asked, want(&vim, "b", 0));
    assert_eq!(
        vim.matched(epoch, &[(0, 1), (3, 1)], Filled::Whole),
        edit(0, 3, "", true)
    );

    // A repeat's `<CR>`: the re-run is a repeat's still, so it sets
    // no new last change of its own.
    let mut vim = on("ab\nxb\n");
    typed(&mut vim, "x");
    for key in keys("d/b").unwrap() {
        vim.key(as_of(key, origin::REPEAT));
    }
    let epoch = vim.epoch();
    assert_eq!(vim.key(as_of(Key::Enter, origin::REPEAT)), want(&vim, "b", 0));
    vim.matched(epoch, &[(0, 1), (3, 1)], Filled::Whole);
    assert_eq!(
        vim.key(tapped(Key::Char('.'))),
        Effect::Fed(vec![Key::Char('x')], true),
        "the repeat's own re-run left the last change alone"
    );
}

#[test]
fn a_reload_ends_the_wait() {
    let mut vim = on("ab\n");
    assert_eq!(typed(&mut vim, "d/b<CR>"), want(&vim, "b", 0));
    assert!(vim.waiting(), "the operator and its search stand");
    // Another Client's edit, read back: the list the wait was of goes
    // at `marked`, and the wait goes with it.
    vim.loaded(b"xy\n");
    assert!(!vim.waiting(), "the reload ended it");
    assert_eq!(vim.showcmd(), "", "the operator went with it");
    typed(&mut vim, "x");
    assert_eq!(vim.text(), "y\n");
    typed(&mut vim, ".");
    assert_eq!(vim.text(), "\n", "the record was the `x` alone");
}

#[test]
fn an_interrupt_puts_pending_back() {
    let mut vim = on("ab\nab\n");
    typed(&mut vim, "x");
    assert_eq!(vim.text(), "b\nab\n");
    assert_eq!(typed(&mut vim, "2d/b<CR>"), want(&vim, "b", 0));
    assert_eq!(vim.showcmd(), "2d");
    vim.rung();
    vim.interrupted();
    assert!(!vim.waiting(), "the wait is over");
    assert_eq!(vim.showcmd(), "", "the operator and its count are back");
    assert!(vim.rung(), "the interrupt rings, so a run ends at it");
    assert_eq!(vim.position(), (0, 0), "the cursor never moved");
    assert_eq!(vim.text(), "b\nab\n", "and nothing was deleted");
    assert_eq!(vim.status(), "", "nothing is said");
    typed(&mut vim, "x");
    assert_eq!(vim.text(), "\nab\n");
    typed(&mut vim, "j.");
    assert_eq!(vim.text(), "\nb\n", "`.` is the change after it, alone");
}

#[test]
fn the_row_words_percent_found_and_eta() {
    // Two entries, neither of which answers a forward search from the
    // cursor over a list still filling: it waits, and the row is of
    // the list it waits on.
    let mut vim = on("ab\nb\n");
    typed(&mut vim, "j");
    let epoch = vim.epoch();
    assert_eq!(typed(&mut vim, "/b<CR>"), want(&vim, "b", 3));
    assert_eq!(
        vim.matched(epoch, &[(3, 1), (1, 1)], Filled::More),
        want(&vim, "b", 3)
    );
    assert!(vim.waiting());
    vim.filling(421_180, 1_000_000, Some(2_500_000_000));
    assert_eq!(vim.status(), "/b  [?/2+]  42.118%  ~3s");
    vim.filling(42_005, 100_000, None);
    assert_eq!(vim.status(), "/b  [?/2+]  42.005%", "the thousandths padded");
    vim.filling(0, 0, None);
    assert_eq!(vim.status(), "/b  [?/2+]", "nothing measured, no percent");
    vim.filling(421_180, 1_000_000, Some(2_500_000_000));
    vim.set("zzz");
    assert_eq!(vim.status(), "E518: Unknown option: zzz", "a message wins");
    vim.rung();
    typed(&mut vim, "qa");
    assert_eq!(
        vim.status(),
        "/b  [?/2+]  42.118%  ~3s recording @a",
        "the record's own words follow the row"
    );

    // A landing over a list still filling keeps the row, and the list
    // is filled on for it.
    let mut vim = on("ab\nb\n");
    let epoch = vim.epoch();
    assert_eq!(typed(&mut vim, "/b<CR>"), want(&vim, "b", 0));
    assert_eq!(vim.matched(epoch, &[(1, 1)], Filled::More), Effect::Moved);
    assert_eq!(vim.position(), (0, 1));
    vim.filling(1_000, 4_000, None);
    assert_eq!(vim.status(), "/b  [?/1+]  25.000%");
    assert!(vim.wants().is_some(), "the row's own list is filled on");
    assert_eq!(typed(&mut vim, "0"), Effect::Moved);
    assert_eq!(vim.status(), "", "and the next key takes the row");
    // `N` reverses the last search's direction, and the row leads with
    // the way it searched, not with the way `/` went.
    assert_eq!(typed(&mut vim, "N"), want(&vim, "b", 0));
    vim.filling(1_000, 4_000, None);
    assert_eq!(vim.status(), "?b  [?/1+]  25.000%");
    vim.matched(vim.epoch(), &[], Filled::Whole);
    assert_eq!(vim.status(), "?b  [1/1]", "a whole list says which match");

    // A landing an operator made shows none: the text is filled again
    // for nobody.
    let mut vim = on("ab\nb\n");
    let epoch = vim.epoch();
    assert_eq!(typed(&mut vim, "y/b<CR>"), want(&vim, "b", 0));
    vim.matched(epoch, &[(1, 1)], Filled::More);
    assert_eq!(vim.status(), "", "a yank's landing shows no row");
    let mut vim = on("ab\nb\n");
    let epoch = vim.epoch();
    assert_eq!(typed(&mut vim, "d/b<CR>"), want(&vim, "b", 0));
    assert_eq!(
        vim.matched(epoch, &[(1, 1)], Filled::More),
        edit(0, 1, "", false)
    );
    assert_eq!(vim.status(), "", "nor does a delete's");
    assert!(vim.wants().is_none(), "and nothing is filled for one");
}

/// The text the count cases search: four `foo`, one on the first
/// line, two on the second and one on the fourth.
const FOUR: &str = "foo a\nb foo foo\nc\nfoo\n";

/// Its four entries, as the fill hands them from the first byte.
const ENTRIES: [(usize, usize); 4] = [(0, 3), (8, 3), (12, 3), (18, 3)];

#[test]
fn the_count_is_the_entries_at_or_before_the_cursor() {
    let mut vim = on(FOUR);
    let epoch = vim.epoch();
    assert_eq!(typed(&mut vim, "/foo<CR>"), want(&vim, "foo", 0));
    assert_eq!(vim.matched(epoch, &ENTRIES, Filled::Whole), Effect::Moved);
    assert_eq!(vim.position(), (1, 2));
    assert_eq!(vim.counted(), Some((2, 4)), "on the match landed on");
    typed(&mut vim, "l");
    assert_eq!(vim.counted(), Some((2, 4)), "inside it, the same one");
    typed(&mut vim, "j");
    assert_eq!(vim.counted(), Some((3, 4)), "between two, the one before");
    typed(&mut vim, "gg");
    assert_eq!(vim.counted(), Some((1, 4)), "on the first of them");

    let mut vim = on("a b\nfoo\nc foo\nd\n");
    let epoch = vim.epoch();
    assert_eq!(typed(&mut vim, "/foo<CR>"), want(&vim, "foo", 0));
    let two = [(4, 3), (10, 3)];
    assert_eq!(vim.matched(epoch, &two, Filled::Whole), Effect::Moved);
    typed(&mut vim, "gg");
    assert_eq!(vim.counted(), Some((0, 2)), "before the first, none");
    typed(&mut vim, "G");
    assert_eq!(vim.counted(), Some((2, 2)), "after the last, all of them");
}

#[test]
fn a_lines_end_reached_from_itself_is_not_counted() {
    // nvim finds a line's matches from column 0, each from the end of
    // the one before it. A match on the line's end that the one
    // before it reached is not found twice, so the count leaves it
    // out — and the fill, which anchors every line, holds it anyway.
    let mut vim = on("ab\ncd\n");
    let epoch = vim.epoch();
    assert_eq!(typed(&mut vim, "/x*<CR>"), want(&vim, "x*", 0));
    let empties = [(0, 0), (1, 0), (2, 0), (3, 0), (4, 0), (5, 0)];
    assert_eq!(vim.matched(epoch, &empties, Filled::Whole), Effect::Moved);
    assert_eq!(vim.counted(), Some((2, 4)), "six entries, four matches");
    for count in [(3, 4), (4, 4), (1, 4)] {
        typed(&mut vim, "n");
        assert_eq!(vim.counted(), Some(count));
    }

    // `$` on each line: the one before it ends a char short of it, so
    // neither is left out.
    let mut vim = on("ab\ncd\n");
    let epoch = vim.epoch();
    assert_eq!(typed(&mut vim, "/$<CR>"), want(&vim, "$", 0));
    let ends = [(2, 0), (5, 0)];
    assert_eq!(vim.matched(epoch, &ends, Filled::Whole), Effect::Moved);
    assert_eq!(vim.counted(), Some((0, 2)), "the landing is a char back");

    // `b\|$`: line 1's `$` stands where the `b` before it ends, so
    // nvim never finds it and the count passes it.
    let mut vim = on("ab\ncd\n");
    let epoch = vim.epoch();
    assert_eq!(typed(&mut vim, "/b\\|$<CR>"), want(&vim, "b\\|$", 0));
    let mixed = [(1, 1), (2, 0), (5, 0)];
    assert_eq!(vim.matched(epoch, &mixed, Filled::Whole), Effect::Moved);
    assert_eq!(vim.counted(), Some((1, 2)), "three entries, two matches");
    typed(&mut vim, "n");
    assert_eq!(vim.position(), (1, 1), "the `d`, the last line's end");
    assert_eq!(vim.counted(), Some((1, 2)));

    // A match that is itself a newline reaches the next line's end,
    // but nvim's per-line search stops at the end of the line it
    // began on, so the one after it is a match of its own.
    let mut vim = on("a\n\n");
    let epoch = vim.epoch();
    assert_eq!(typed(&mut vim, "/\\n<CR>"), want(&vim, "\\n", 0));
    let newlines = [(1, 1), (2, 1)];
    assert_eq!(vim.matched(epoch, &newlines, Filled::Whole), Effect::Moved);
    assert_eq!(vim.position(), (1, 0));
    assert_eq!(vim.counted(), Some((2, 2)), "neither is passed");

    // The entry before it ends three chars short of the line's end,
    // so nvim finds that end too.
    let mut vim = on("abc\n");
    let epoch = vim.epoch();
    assert_eq!(typed(&mut vim, "/a\\|$<CR>"), want(&vim, "a\\|$", 0));
    let apart = [(0, 1), (3, 0)];
    assert_eq!(vim.matched(epoch, &apart, Filled::Whole), Effect::Moved);
    assert_eq!(vim.position(), (0, 2));
    assert_eq!(vim.counted(), Some((1, 2)), "the end is not reached");
}

#[test]
fn the_row_is_a_number_once_the_list_is_whole() {
    let mut vim = on(FOUR);
    let epoch = vim.epoch();
    assert_eq!(typed(&mut vim, "/foo<CR>"), want(&vim, "foo", 0));
    assert_eq!(vim.matched(epoch, &ENTRIES[..2], Filled::More), Effect::Moved);
    vim.filling(1_000, 4_000, None);
    assert_eq!(vim.status(), "/foo  [?/2+]  25.000%", "no rank while it fills");
    assert_eq!(
        vim.matched(epoch, &ENTRIES[2..], Filled::Whole),
        Effect::Nothing
    );
    assert_eq!(vim.status(), "/foo  [2/4]", "a number, and nothing after it");
    assert_eq!(typed(&mut vim, "N"), Effect::Moved);
    assert_eq!(vim.status(), "?foo  [1/4]", "the way it searched leads");

    let mut vim = on(&"a\n".repeat(1200));
    let epoch = vim.epoch();
    assert_eq!(typed(&mut vim, "/a<CR>"), want(&vim, "a", 0));
    let many: Vec<(usize, usize)> = (0..1200).map(|n| (n * 2, 1)).collect();
    assert_eq!(vim.matched(epoch, &many, Filled::Whole), Effect::Moved);
    assert_eq!(vim.status(), "/a  [2/1200]", "there is no cap");
}

#[test]
fn the_row_goes_at_the_next_key_and_an_operator_shows_none() {
    let mut vim = on(FOUR);
    let epoch = vim.epoch();
    assert_eq!(typed(&mut vim, "/foo<CR>"), want(&vim, "foo", 0));
    vim.matched(epoch, &ENTRIES, Filled::Whole);
    assert_eq!(vim.status(), "/foo  [2/4]");
    assert_eq!(typed(&mut vim, "l"), Effect::Moved);
    assert_eq!(vim.status(), "", "the row goes at the next key");
    assert!(vim.wants().is_none(), "and an idle frame reads nothing");

    let mut vim = on(FOUR);
    let epoch = vim.epoch();
    assert_eq!(typed(&mut vim, "d/foo<CR>"), want(&vim, "foo", 0));
    vim.matched(epoch, &ENTRIES, Filled::Whole);
    assert_eq!(vim.text(), "foo foo\nc\nfoo\n", "the delete ran");
    assert_eq!(vim.status(), "", "and its landing shows no count");
    assert!(vim.wants().is_none());

    let mut vim = on(FOUR);
    let epoch = vim.epoch();
    assert_eq!(typed(&mut vim, "/zzz<CR>"), want(&vim, "zzz", 0));
    vim.matched(epoch, &[], Filled::Whole);
    assert_eq!(vim.status(), "E486: Pattern not found: zzz");
    assert_eq!(vim.counted(), Some((0, 0)), "none found is none of none");
}

#[test]
fn the_count_is_unknown_until_whole_and_under_another_key() {
    let mut vim = on(FOUR);
    let epoch = vim.epoch();
    assert_eq!(typed(&mut vim, "/foo<CR>"), want(&vim, "foo", 0));
    vim.matched(epoch, &ENTRIES[..2], Filled::More);
    assert_eq!(vim.counted(), None, "a list still filling has no rank");
    vim.matched(epoch, &ENTRIES[2..], Filled::Whole);
    assert_eq!(vim.counted(), Some((2, 4)));
    typed(&mut vim, "x");
    assert_eq!(vim.counted(), None, "the edit dropped the list");

    let mut vim = on(FOUR);
    let epoch = vim.epoch();
    assert_eq!(typed(&mut vim, "/foo<CR>"), want(&vim, "foo", 0));
    vim.matched(epoch, &ENTRIES, Filled::Whole);
    assert_eq!(vim.counted(), Some((2, 4)));
    assert_eq!(vim.set("ic"), Effect::Moved);
    assert_eq!(vim.counted(), None, "the list is of another key now");
}

/// The text 02.5's cases stand on: `foo` on line 2 and `foo` again on
/// line 3, so a typed pattern has two entries to choose between.
const T: &str = "abc\nfoo bar\nbaz foo\n";

/// Every entry of `o` in [`T`], in the text's order.
const OS: [(usize, usize); 4] = [(5, 1), (6, 1), (17, 1), (18, 1)];

/// The list the open line asks for, answered whole with `entries`.
/// `None` where the line asked for none: a list Vim still holds
/// under the line's own key is not asked for again.
fn answering(vim: &mut Vim, entries: &[(usize, usize)]) -> Option<Want> {
    let asked = vim.wants()?;
    vim.matched(asked.epoch, entries, Filled::Whole);
    Some(asked)
}

#[test]
fn a_typed_pattern_is_wanted_and_the_cursor_follows() {
    let mut vim = on(T);
    assert_eq!(typed(&mut vim, "/"), Effect::Moved);
    assert!(vim.wants().is_none(), "an empty line wants nothing");
    assert_eq!(typed(&mut vim, "f"), Effect::Moved);
    let asked = vim.wants();
    assert_eq!(
        asked.map(Effect::Search),
        Some(want(&vim, "f", 0)),
        "the line so far, begun where it was opened"
    );
    assert_eq!(
        vim.matched(vim.epoch(), &[(4, 1), (16, 1)], Filled::Whole),
        Effect::Moved
    );
    assert_eq!(vim.position(), (1, 0), "the cursor on the first entry");
    let spans = vim.view(10).spans;
    let marks: Vec<(u64, u64)> = spans
        .iter()
        .filter(|span| span.style == MATCH)
        .map(|span| (span.from, span.to))
        .collect();
    assert_eq!(marks, [(4, 5)], "one MATCH span, `hlsearch` off");
    assert!(vim.wants().is_none(), "an answered line wants no more");

    assert_eq!(typed(&mut vim, "o"), Effect::Moved);
    assert_eq!(vim.position(), (0, 0), "back at the origin until answered");
    let asked = vim.wants();
    assert_eq!(asked.map(Effect::Search), Some(want(&vim, "fo", 0)));
    vim.matched(vim.epoch(), &[(4, 2), (16, 2)], Filled::Whole);
    assert_eq!(vim.position(), (1, 0));

    typed(&mut vim, "<BS><BS>");
    assert_eq!(vim.position(), (0, 0), "the empty line is the origin");
    assert!(vim.wants().is_none(), "and wants nothing");

    // An entry on a line's end is shown at that line's last char,
    // which is where a landing puts the cursor.
    let mut vim = on(T);
    typed(&mut vim, "/$");
    answering(&mut vim, &[(3, 0), (11, 0), (19, 0)]);
    assert_eq!(vim.position(), (0, 2), "the last char, not the end");

    // `?` looks the way the line it was opened with says: back to
    // the entry before the origin, where `/` would take the one after.
    let mut vim = on(T);
    typed(&mut vim, "G?foo");
    answering(&mut vim, &[(4, 3), (16, 3)]);
    assert_eq!(vim.position(), (1, 0), "the entry before the last line");
}

#[test]
fn every_way_out_puts_the_cursor_and_the_top_back() {
    let mut text = String::new();
    for n in 1..=60 {
        let _ = writeln!(text, "{}", if n == 55 { "foo" } else { "line" });
    }
    let at = text.find("foo").unwrap();
    let mut vim = on(&text);
    assert_eq!(vim.view(10).top, 0);
    for out in ["<Esc>", "<C-c>", "<BS><BS><BS><BS>"] {
        typed(&mut vim, "/foo");
        answering(&mut vim, &[(at, 3)]);
        assert_eq!(vim.position(), (54, 0), "{out}: the match");
        assert!(vim.view(10).top > 0, "{out}: the window followed it");
        assert_eq!(typed(&mut vim, out), Effect::Moved);
        assert_eq!(vim.mode(), Mode::Normal, "{out}: the prompt is gone");
        assert_eq!(vim.position(), (0, 0), "{out}: the cursor is back");
        assert_eq!(vim.view(10).top, 0, "{out}: and the window with it");
    }

    // The text loaded again under the open line: the origin the way
    // out goes back to is resettled, as a reload leaves a cursor.
    typed(&mut vim, "G$/foo");
    assert!(vim.loaded(b"ab\n"), "the text is another");
    typed(&mut vim, "o");
    assert_eq!(vim.position(), (0, 2), "the origin is held inside it");
    typed(&mut vim, "<Esc>");
    assert_eq!(vim.position(), (0, 1), "and comes back onto a char of it");
}

#[test]
fn no_entry_and_a_refused_pattern_say_nothing() {
    let mut vim = on(T);
    typed(&mut vim, "/zzz");
    let asked = answering(&mut vim, &[]).expect("the typed line is wanted");
    assert_eq!(asked.pattern, "zzz");
    assert_eq!(vim.position(), (0, 0), "nothing found, nothing moved");
    assert_eq!(vim.status(), "/zzz", "the status is the line");
    assert!(!vim.rung());
    assert!(vim.wants().is_none(), "a whole list is asked for no more");

    typed(&mut vim, "<Esc>/\\(");
    let asked = vim.wants().expect("a pattern that does not parse is asked");
    vim.matched(asked.epoch, &[], Filled::Refused);
    assert_eq!(vim.position(), (0, 0));
    assert_eq!(vim.status(), "/\\(");
    assert!(!vim.rung(), "a refused open is silence");
    assert!(vim.wants().is_none(), "one refusal is enough");
    assert_eq!(typed(&mut vim, "<CR>"), Effect::Moved, "the word is a change");
    assert_eq!(vim.status(), "E486: Pattern not found: \\(");
    assert!(vim.rung(), "the word rings");
    assert!(vim.wants().is_none(), "and asks for nothing");
}

#[test]
fn enter_lands_from_the_origin() {
    let mut vim = on(T);
    typed(&mut vim, "3/o");
    answering(&mut vim, &OS);
    assert_eq!(vim.position(), (2, 5), "the third entry, unanswered before");
    assert_eq!(typed(&mut vim, "<CR>"), Effect::Moved);
    assert_eq!(vim.position(), (2, 5), "where the preview stood");
    typed(&mut vim, "``");
    assert_eq!(vim.position(), (0, 0), "one jump, pushed from the origin");

    let mut vim = on(T);
    typed(&mut vim, "d/foo");
    let asked = answering(&mut vim, &[(4, 3), (16, 3)]).expect("it is wanted");
    assert_eq!(asked.from, 0, "the list was begun where the line opened");
    assert_eq!(vim.position(), (1, 0), "the match under a waiting operator");
    assert_eq!(typed(&mut vim, "<CR>"), edit(0, 4, "", false));
    assert_eq!(vim.text(), "foo bar\nbaz foo\n", "deleted from the origin");

    let mut vim = on(T);
    typed(&mut vim, "/foo");
    answering(&mut vim, &[(4, 3), (16, 3)]);
    typed(&mut vim, "<CR>gg/ba");
    answering(&mut vim, &[(8, 2), (12, 2)]);
    assert_eq!(vim.position(), (1, 4), "the typed line's own match");
    assert_eq!(typed(&mut vim, "<Esc>"), Effect::Moved);
    assert_eq!(vim.position(), (0, 0));
    assert_eq!(typed(&mut vim, "n"), want(&vim, "foo", 0), "the old pattern");
    vim.matched(vim.epoch(), &[(4, 3), (16, 3)], Filled::Whole);
    assert_eq!(vim.position(), (1, 0));
}

#[test]
fn a_selection_follows_the_typed_pattern() {
    let mut vim = on(T);
    typed(&mut vim, "v/foo");
    answering(&mut vim, &[(4, 3), (16, 3)]);
    assert_eq!(vim.position(), (1, 0));
    let spans = vim.view(10).spans;
    let selected: Vec<(u64, u64)> = spans
        .iter()
        .filter(|span| span.style == SELECTION)
        .map(|span| (span.from, span.to))
        .collect();
    assert_eq!(
        (selected.first().map(|s| s.0), selected.last().map(|s| s.1)),
        (Some(0), Some(5)),
        "the selection stretches to the match: {selected:?}"
    );
    assert_eq!(typed(&mut vim, "<Esc>"), Effect::Moved);
    assert_eq!(vim.mode(), Mode::Visual(Visual::Char), "visual stands");
    assert_eq!(vim.position(), (0, 0), "at the origin");
}

#[test]
fn enter_before_the_answer_waits() {
    let mut vim = on(T);
    typed(&mut vim, "/foo");
    let asked = vim.wants().expect("the typed line is wanted");
    assert_eq!(vim.matched(asked.epoch, &[], Filled::More), Effect::Moved);
    assert_eq!(vim.position(), (0, 0), "nothing found yet");
    assert_eq!(typed(&mut vim, "<CR>"), want(&vim, "foo", 0));
    assert!(vim.waiting(), "the Enter waits on the list the preview began");
    assert_eq!(
        vim.matched(vim.epoch(), &[(4, 3), (16, 3)], Filled::Whole),
        Effect::Moved
    );
    assert_eq!(vim.position(), (1, 0));
}

#[test]
fn ctrl_g_and_ctrl_t_do_nothing() {
    let mut vim = on(T);
    typed(&mut vim, "/foo");
    answering(&mut vim, &[(4, 3), (16, 3)]);
    assert_eq!(vim.position(), (1, 0));
    for key in ["<C-g>", "<C-t>"] {
        assert_eq!(typed(&mut vim, key), Effect::Nothing, "{key}");
        assert_eq!(vim.status(), "/foo", "{key}: the line stands");
        assert_eq!(vim.position(), (1, 0), "{key}: and the match with it");
    }
}
