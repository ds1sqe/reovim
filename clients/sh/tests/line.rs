//! The line editor: keys in, acts out, no terminal and no tree. The
//! history and the cuts are the line's; the screen is the host's.

use reovim_client_api::Key;
use reovim_client_sh::{Act, Completion, Line};

/// The chars of `text`, one key at a time.
fn typed(line: &mut Line, text: &str) {
    for c in text.chars() {
        assert_eq!(line.key(Key::Char(c)), Act::Redraw);
    }
}

#[test]
fn keys_edit_submit_and_recall() {
    let mut line = Line::default();
    typed(&mut line, "sat");
    assert_eq!(line.key(Key::Left), Act::Redraw);
    assert_eq!(line.key(Key::Left), Act::Redraw);
    typed(&mut line, "t");
    assert_eq!(line.text(), "stat");
    assert_eq!(line.key(Key::End), Act::Redraw);
    assert_eq!(line.key(Key::End), Act::Nothing);
    assert_eq!(line.key(Key::Right), Act::Nothing);
    typed(&mut line, " /x");
    assert_eq!(line.key(Key::Backspace), Act::Redraw);
    assert_eq!(line.render("> "), ("> stat /".to_string(), 8));
    assert_eq!(line.key(Key::Home), Act::Redraw);
    assert_eq!(line.key(Key::Backspace), Act::Nothing);
    assert_eq!(line.key(Key::Tab), Act::Complete);
    assert_eq!(line.key(Key::Esc), Act::Nothing);
    assert_eq!(line.key(Key::Enter), Act::Submit("stat /".to_string()));
    assert_eq!(line.text(), "");

    typed(&mut line, "ls /");
    assert_eq!(line.key(Key::Enter), Act::Submit("ls /".to_string()));
    assert_eq!(line.key(Key::Enter), Act::Submit(String::new()));
    assert_eq!(line.key(Key::Up), Act::Redraw);
    assert_eq!(line.text(), "ls /");
    assert_eq!(line.key(Key::Up), Act::Redraw);
    assert_eq!(line.text(), "stat /");
    assert_eq!(line.key(Key::Up), Act::Nothing);
    assert_eq!(line.key(Key::Down), Act::Redraw);
    assert_eq!(line.text(), "ls /");
    assert_eq!(line.key(Key::Down), Act::Redraw);
    assert_eq!(line.text(), "");
    assert_eq!(line.key(Key::Down), Act::Nothing);
    assert_eq!(line.key(Key::Ctrl('c')), Act::Quit);
    assert_eq!(line.key(Key::Ctrl('d')), Act::Quit);
    assert_eq!(line.key(Key::Ctrl('x')), Act::Nothing);
}

#[test]
fn alt_is_nothing_to_the_line() {
    let mut line = Line::default();
    typed(&mut line, "cat");
    assert_eq!(line.key(Key::Alt('x')), Act::Nothing);
    assert_eq!(line.text(), "cat");
}

#[test]
fn a_completion_replaces_the_token() {
    let mut line = Line::default();
    typed(&mut line, "l");
    let one = Completion { from: 0, candidates: vec!["ls".to_string()] };
    line.apply(&one);
    assert_eq!(line.text(), "ls ");
    typed(&mut line, "/mod");
    let dir = Completion { from: 3, candidates: vec!["/module/".to_string()] };
    line.apply(&dir);
    assert_eq!(line.text(), "ls /module/");
    let several = Completion {
        from: 3,
        candidates: vec!["/module/fs".to_string(), "/module/fsx".to_string()],
    };
    line.apply(&several);
    assert_eq!(line.text(), "ls /module/fs");
    assert_eq!(line.render("").1, 13);
    line.apply(&Completion::default());
    assert_eq!(line.text(), "ls /module/fs");

    // What `apply` puts in by itself: a token from `from` on, the
    // cursor after it, and nothing at all for `put(0, "")`.
    line.put(3, "/sys");
    assert_eq!(line.text(), "ls /sys");
    assert_eq!(line.render("").1, 7);
    line.put(0, "");
    assert_eq!(line.text(), "");
    assert_eq!(line.render("> "), ("> ".to_string(), 2));
}

#[test]
fn the_keys_cut_the_line_and_the_screen_is_the_hosts() {
    let mut line = Line::default();
    typed(&mut line, "ls /module/fs");

    // Ctrl-W takes the token before the cursor, the spaces with it.
    assert_eq!(line.key(Key::Ctrl('w')), Act::Redraw);
    assert_eq!(line.text(), "ls ");
    assert_eq!(line.key(Key::Ctrl('w')), Act::Redraw);
    assert_eq!(line.text(), "");
    assert_eq!(line.key(Key::Ctrl('w')), Act::Nothing);

    // Ctrl-A and Ctrl-E are Home and End; Ctrl-U cuts to the start.
    typed(&mut line, "cat note");
    assert_eq!(line.key(Key::Ctrl('a')), Act::Redraw);
    assert_eq!(line.key(Key::Ctrl('a')), Act::Nothing);
    assert_eq!(line.key(Key::Ctrl('e')), Act::Redraw);
    assert_eq!(line.key(Key::Ctrl('e')), Act::Nothing);
    assert_eq!(line.render("/ > "), ("/ > cat note".to_string(), 12));
    assert_eq!(line.key(Key::Left), Act::Redraw);
    assert_eq!(line.key(Key::Ctrl('u')), Act::Redraw);
    assert_eq!(line.text(), "e");
    assert_eq!(line.key(Key::Ctrl('u')), Act::Nothing);

    // Ctrl-D takes the char under the cursor while there is a line,
    // and ends the session once there is not. Ctrl-L is the host's.
    assert_eq!(line.key(Key::Ctrl('d')), Act::Redraw);
    assert_eq!(line.text(), "");
    assert_eq!(line.key(Key::Ctrl('d')), Act::Quit);
    typed(&mut line, "ab");
    assert_eq!(line.key(Key::Ctrl('d')), Act::Nothing, "at the end");
    assert_eq!(line.key(Key::Home), Act::Redraw);
    assert_eq!(line.key(Key::Ctrl('d')), Act::Redraw);
    assert_eq!(line.text(), "b");
    assert_eq!(line.key(Key::Ctrl('l')), Act::Clear);
    assert_eq!(line.text(), "b", "the screen goes, the line stays");
    assert_eq!(line.key(Key::Ctrl('c')), Act::Quit);
    assert_eq!(line.key(Key::Ctrl('x')), Act::Nothing);
}

#[test]
fn up_walks_the_lines_that_begin_as_this_one_does() {
    let mut line = Line::default();
    for command in ["ls -l", "cat note", "ls /sys"] {
        typed(&mut line, command);
        assert_eq!(line.key(Key::Enter), Act::Submit(command.to_string()));
    }

    // An empty line walks them all, newest first.
    assert_eq!(line.key(Key::Up), Act::Redraw);
    assert_eq!(line.text(), "ls /sys");
    assert_eq!(line.key(Key::Up), Act::Redraw);
    assert_eq!(line.text(), "cat note");
    assert_eq!(line.key(Key::Down), Act::Redraw);
    assert_eq!(line.text(), "ls /sys");
    assert_eq!(line.key(Key::Down), Act::Redraw);
    assert_eq!(line.text(), "", "past the newest is the line as it was");

    // A line that has something in it walks only what begins with it,
    // and past the newest of those is that beginning again.
    typed(&mut line, "ls");
    assert_eq!(line.key(Key::Up), Act::Redraw);
    assert_eq!(line.text(), "ls /sys");
    assert_eq!(line.key(Key::Up), Act::Redraw);
    assert_eq!(line.text(), "ls -l");
    assert_eq!(line.key(Key::Up), Act::Nothing);
    assert_eq!(line.key(Key::Down), Act::Redraw);
    assert_eq!(line.text(), "ls /sys");
    assert_eq!(line.key(Key::Down), Act::Redraw);
    assert_eq!(line.text(), "ls");

    // Editing the line starts the next walk from what it holds now.
    assert_eq!(line.key(Key::Backspace), Act::Redraw);
    assert_eq!(line.key(Key::Up), Act::Redraw);
    assert_eq!(line.text(), "ls /sys");
    typed(&mut line, "x");
    assert_eq!(line.key(Key::Up), Act::Nothing);
    assert_eq!(line.text(), "ls /sysx");
}
