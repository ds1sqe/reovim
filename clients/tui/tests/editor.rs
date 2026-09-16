//! The editor alone: keys in, text and frames out.

use reovim_client::Key;
use reovim_client_tui::{Editor, Effect};

fn edit(at: usize, len: usize, text: &str) -> Effect {
    Effect::Edit { at, len, text: text.to_string() }
}

/// Every char goes in at the cursor as one edit.
fn typed(editor: &mut Editor, text: &str) {
    for c in text.chars() {
        let key = if c == '\n' { Key::Enter } else { Key::Char(c) };
        let at = editor.text().len();
        assert_eq!(editor.key(key), edit(at, 0, &c.to_string()));
    }
}

#[test]
fn keys_edit_and_move() {
    let mut editor = Editor::default();
    typed(&mut editor, "ab\ncd");
    assert_eq!(editor.text(), "ab\ncd");
    assert_eq!(editor.position(), (1, 2));
    assert_eq!(editor.key(Key::Backspace), edit(4, 1, ""));
    assert_eq!(editor.text(), "ab\nc");
    assert_eq!(editor.key(Key::Right), Effect::Nothing);
    assert_eq!(editor.key(Key::Up), Effect::Moved);
    assert_eq!(editor.position(), (0, 1));
    assert_eq!(editor.key(Key::End), Effect::Moved);
    assert_eq!(editor.position(), (0, 2));
    assert_eq!(editor.key(Key::Down), Effect::Moved);
    assert_eq!(editor.position(), (1, 1), "the column is clamped");
    assert_eq!(editor.key(Key::Down), Effect::Nothing);
    assert_eq!(editor.key(Key::Home), Effect::Moved);
    assert_eq!(editor.key(Key::Home), Effect::Nothing);
    assert_eq!(editor.key(Key::Left), Effect::Moved);
    assert_eq!(editor.position(), (0, 2));
    assert_eq!(editor.key(Key::Char('é')), edit(2, 0, "é"));
    assert_eq!(editor.text(), "abé\nc");
    assert_eq!(editor.key(Key::Left), Effect::Moved);
    assert_eq!(editor.key(Key::Backspace), edit(1, 1, ""));
    assert_eq!(editor.text(), "aé\nc");
    assert_eq!(editor.key(Key::Up), Effect::Nothing);
    assert_eq!(editor.key(Key::Tab), Effect::Nothing);
    assert_eq!(editor.key(Key::Ctrl('z')), Effect::Undo);
    assert_eq!(editor.key(Key::Ctrl('y')), Effect::Redo);
    assert_eq!(editor.key(Key::Ctrl('s')), Effect::Save);
    assert_eq!(editor.key(Key::Ctrl('q')), Effect::Quit);
    assert_eq!(editor.key(Key::Ctrl('c')), Effect::Abort);
    assert_eq!(editor.key(Key::Esc), Effect::Nothing);
    assert_eq!(editor.key(Key::Ctrl('x')), Effect::Nothing);
}

#[test]
fn loaded_replaces_and_clamps() {
    let mut editor = Editor::default();
    typed(&mut editor, "hello");
    assert!(!editor.loaded(b"hello"), "the same text");
    assert!(editor.loaded(b"hi"));
    assert_eq!(editor.position(), (0, 2), "clamped to the end");
    assert!(editor.loaded("aé".as_bytes()));
    assert_eq!(editor.position(), (0, 1), "clamped to a char boundary");
    assert!(editor.loaded(b"\xff"));
    assert_eq!(editor.text(), "\u{fffd}", "lossy");
}

#[test]
fn a_frame_keeps_the_cursor_on_screen_and_cuts_lines() {
    let mut editor = Editor::default();
    typed(&mut editor, "1\n2\n3\n4\n5 is long");
    let frame = editor.frame(4, 3);
    assert_eq!(frame.lines, vec!["4", "5 is"]);
    assert_eq!(frame.cursor, (1, 3), "the last column when past it");
    assert_eq!(frame.status, "5:10");
    for _ in 0..4 {
        editor.key(Key::Up);
    }
    let frame = editor.frame(4, 3);
    assert_eq!(frame.lines, vec!["1", "2"]);
    assert_eq!(frame.cursor, (0, 1));
    editor.key(Key::Down);
    editor.key(Key::Down);
    let frame = editor.frame(80, 3);
    assert_eq!(frame.lines, vec!["2", "3"]);
    assert_eq!(frame.cursor, (1, 1));
    assert_eq!(editor.frame(80, 1).lines, vec!["3"], "one row still shows");
}
