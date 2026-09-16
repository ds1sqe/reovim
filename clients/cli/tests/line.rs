//! The line editor: keys in, acts out, no terminal.

use reovim_client::Key;
use reovim_client_cli::{Act, Completion, Line};

fn typed(line: &mut Line, text: &str) {
    for c in text.chars() {
        assert_eq!(line.key(Key::Char(c)), Act::Redraw);
    }
}

#[test]
fn keys_edit_submit_and_recall() {
    let mut line = Line::default();
    typed(&mut line, "lokup");
    assert_eq!(line.key(Key::Left), Act::Redraw);
    assert_eq!(line.key(Key::Left), Act::Redraw);
    assert_eq!(line.key(Key::Left), Act::Redraw);
    typed(&mut line, "o");
    assert_eq!(line.text(), "lookup");
    assert_eq!(line.key(Key::End), Act::Redraw);
    assert_eq!(line.key(Key::End), Act::Nothing);
    assert_eq!(line.key(Key::Right), Act::Nothing);
    typed(&mut line, " /x");
    assert_eq!(line.key(Key::Backspace), Act::Redraw);
    assert_eq!(line.render("> "), ("> lookup /".to_string(), 10));
    assert_eq!(line.key(Key::Home), Act::Redraw);
    assert_eq!(line.key(Key::Backspace), Act::Nothing);
    assert_eq!(line.key(Key::Tab), Act::Complete);
    assert_eq!(line.key(Key::Esc), Act::Nothing);
    assert_eq!(line.key(Key::Enter), Act::Submit("lookup /".to_string()));
    assert_eq!(line.text(), "");

    typed(&mut line, "info /");
    assert_eq!(line.key(Key::Enter), Act::Submit("info /".to_string()));
    assert_eq!(line.key(Key::Enter), Act::Submit(String::new()));
    assert_eq!(line.key(Key::Up), Act::Redraw);
    assert_eq!(line.text(), "info /");
    assert_eq!(line.key(Key::Up), Act::Redraw);
    assert_eq!(line.text(), "lookup /");
    assert_eq!(line.key(Key::Up), Act::Nothing);
    assert_eq!(line.key(Key::Down), Act::Redraw);
    assert_eq!(line.text(), "info /");
    assert_eq!(line.key(Key::Down), Act::Redraw);
    assert_eq!(line.text(), "");
    assert_eq!(line.key(Key::Down), Act::Nothing);
    assert_eq!(line.key(Key::Ctrl('c')), Act::Quit);
    assert_eq!(line.key(Key::Ctrl('d')), Act::Quit);
    assert_eq!(line.key(Key::Ctrl('x')), Act::Nothing);
}

#[test]
fn a_completion_replaces_the_word() {
    let mut line = Line::default();
    typed(&mut line, "lo");
    let one = Completion { from: 0, candidates: vec!["lookup".to_string()] };
    line.apply(&one);
    assert_eq!(line.text(), "lookup ");
    typed(&mut line, "/mod");
    let dir = Completion { from: 7, candidates: vec!["/module/".to_string()] };
    line.apply(&dir);
    assert_eq!(line.text(), "lookup /module/");
    let several = Completion {
        from: 7,
        candidates: vec!["/module/fs".to_string(), "/module/fsx".to_string()],
    };
    line.apply(&several);
    assert_eq!(line.text(), "lookup /module/fs");
    assert_eq!(line.render("").1, 17);
    line.apply(&Completion::default());
    assert_eq!(line.text(), "lookup /module/fs");
}
