//! Vim alone: keys in, text and frames out.

use reovim_client_vim::notation::keys;
use reovim_client_vim::{Effect, Key, Mode, Quit, Vim};

fn edit(at: usize, len: usize, text: &str, join: bool) -> Effect {
    Effect::Edit { at, len, text: text.to_string(), join }
}

/// Every key of `text` in Vim notation; the last effect.
fn typed(vim: &mut Vim, text: &str) -> Effect {
    let mut last = Effect::Nothing;
    for key in keys(text).unwrap() {
        last = vim.key(key);
    }
    last
}

/// A Vim on `text`, in normal mode at the start.
fn on(text: &str) -> Vim {
    let mut vim = Vim::default();
    vim.loaded(text.as_bytes());
    vim
}

#[test]
fn insert_edits_join_and_esc_steps_back() {
    let mut vim = on("");
    assert_eq!(vim.key(Key::Char('i')), Effect::Moved);
    assert_eq!(vim.mode(), Mode::Insert);
    assert_eq!(vim.key(Key::Char('a')), edit(0, 0, "a", false));
    assert_eq!(vim.key(Key::Char('b')), edit(1, 0, "b", true));
    assert_eq!(vim.key(Key::Enter), edit(2, 0, "\n", true));
    assert_eq!(vim.key(Key::Char('c')), edit(3, 0, "c", true));
    assert_eq!(vim.position(), (1, 1));
    assert_eq!(vim.key(Key::Backspace), edit(3, 1, "", true));
    assert_eq!(vim.key(Key::Backspace), edit(2, 1, "", true), "across");
    assert_eq!(vim.text(), "ab");
    assert_eq!(vim.key(Key::Left), Effect::Moved, "an arrow ends the step");
    assert_eq!(vim.key(Key::Char('é')), edit(1, 0, "é", false));
    assert_eq!(vim.key(Key::Esc), Effect::Moved);
    assert_eq!(vim.mode(), Mode::Normal);
    assert_eq!(vim.position(), (0, 1), "back onto the char inserted");
    assert_eq!(vim.key(Key::Esc), Effect::Nothing);
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
fn a_frame_keeps_the_cursor_on_screen_and_cuts_lines() {
    let mut vim = on("1\n2\n3\n4\n5 is long");
    typed(&mut vim, "4j$");
    let frame = vim.frame(4, 3);
    assert_eq!(frame.lines, vec!["4", "5 is"]);
    assert_eq!(frame.cursor, (1, 3), "the last column when past it");
    assert_eq!(frame.status, "");
    typed(&mut vim, "4k");
    let frame = vim.frame(4, 3);
    assert_eq!(frame.lines, vec!["1", "2"]);
    assert_eq!(frame.cursor, (0, 0));
    typed(&mut vim, "2j");
    let frame = vim.frame(80, 3);
    assert_eq!(frame.lines, vec!["2", "3"]);
    assert_eq!(frame.cursor, (1, 0));
    assert_eq!(vim.frame(80, 1).lines, vec!["3"], "one row still shows");
    typed(&mut vim, "i");
    assert_eq!(vim.frame(80, 3).status, "-- INSERT --");
}

#[test]
fn notation_names_the_keys_vim_names() {
    let parsed = keys("a<Esc>:wq<CR><C-r><BS><lt>é").unwrap();
    let expected = [
        Key::Char('a'),
        Key::Esc,
        Key::Char(':'),
        Key::Char('w'),
        Key::Char('q'),
        Key::Enter,
        Key::Ctrl('r'),
        Key::Backspace,
        Key::Char('<'),
        Key::Char('é'),
    ];
    assert_eq!(parsed, expected);
    assert_eq!(keys("<Nope>"), None);
    assert_eq!(keys("<C-"), None);
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
    assert_eq!(vim.position(), (1, 0));
    assert_eq!(typed(&mut vim, "dd"), edit(4, 6, "", false), "the last");
    assert_eq!(vim.text(), "one\n");
    assert_eq!(vim.position(), (0, 0));
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
    vim.goto_line(5);
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
    assert_eq!(vim.fix_eol(), Some(edit(3, 0, "\n", false)));
    assert_eq!(vim.fix_eol(), None);
    assert_eq!(vim.text(), "abc\n");
    assert_eq!(vim.frame(80, 24).lines, vec!["abc"], "one line, ended");
    assert_eq!(typed(&mut vim, "j"), Effect::Nothing);
    let mut empty = on("");
    assert_eq!(empty.fix_eol(), None);
    assert_eq!(empty.frame(80, 24).lines, vec![""]);
}

#[test]
fn the_command_line_runs_the_quit_commands_and_refuses_the_rest() {
    let mut vim = on("abc\n");
    assert_eq!(typed(&mut vim, ":"), Effect::Moved);
    assert_eq!(vim.mode(), Mode::Command);
    assert_eq!(typed(&mut vim, "wq"), Effect::Moved);
    assert_eq!(vim.frame(80, 24).status, ":wq");
    assert_eq!(typed(&mut vim, "<BS><BS>"), Effect::Moved);
    assert_eq!(vim.frame(80, 24).status, ":");
    assert_eq!(typed(&mut vim, "<BS>"), Effect::Moved, "leaves");
    assert_eq!(vim.mode(), Mode::Normal);
    assert_eq!(typed(&mut vim, ":q<Esc>"), Effect::Moved);
    assert_eq!(vim.mode(), Mode::Normal);
    assert_eq!(typed(&mut vim, ":<CR>"), Effect::Moved, "an empty line");

    assert_eq!(typed(&mut vim, ":w<CR>"), Effect::Save);
    assert_eq!(typed(&mut vim, ":q<CR>"), Effect::Quit(Quit::Ask));
    assert_eq!(typed(&mut vim, ":q!<CR>"), Effect::Quit(Quit::Force));
    assert_eq!(typed(&mut vim, ":wq<CR>"), Effect::Quit(Quit::Write));
    assert_eq!(typed(&mut vim, ": x <CR>"), Effect::Quit(Quit::Write));
    assert_eq!(typed(&mut vim, ":cq<CR>"), Effect::Quit(Quit::Abort));
    assert_eq!(typed(&mut vim, "ZZ"), Effect::Quit(Quit::Write));
    assert_eq!(typed(&mut vim, "ZQ"), Effect::Quit(Quit::Force));
    assert_eq!(typed(&mut vim, "Zx"), Effect::Nothing);

    assert_eq!(typed(&mut vim, ":xyz<CR>"), Effect::Moved);
    let status = vim.frame(80, 24).status;
    assert_eq!(status, "E492: Not an editor command: xyz");
    assert_eq!(typed(&mut vim, "<Esc>"), Effect::Moved, "the message goes");
    assert_eq!(vim.frame(80, 24).status, "");
    vim.notice("E37: No write since last change (add ! to override)");
    assert!(vim.frame(80, 24).status.starts_with("E37"));
    assert_eq!(typed(&mut vim, "<C-s>"), Effect::Moved, "no longer a key");
    assert_eq!(typed(&mut vim, "<C-q>"), Effect::Nothing);
}
