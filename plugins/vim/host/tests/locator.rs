//! The point read back: `goto`, `reovim FILE:LOCATOR`, `:e
//! FILE:LOCATOR` and `gF`.
//!
//! The locator is the view's, not the app's and not the tree's: text's
//! own grammar is `LINE`, `LINE:COL`, `FROM-TO` and
//! `LINE:COL-LINE:COL`, a range landing at its start, and anything
//! else is another domain's, which this view says it cannot read.

mod common;

use std::fmt::Write as _;
use std::rc::Rc;

use reovim_client::Host;
use reovim_client::dev::{self, Clip, Table, Want};
use reovim_client_tui::Tui;
use reovim_plugin_vim_host::Editor;
use reovim_rig::{Rig, frame, listed, opened, said, typed};

/// Twenty lines of seven bytes each, so a column past the fourth is
/// on every one of them.
fn twenty() -> Vec<u8> {
    let written = |mut out: String, n: u32| {
        let _ = writeln!(out, "line-{n:02}");
        out
    };
    (1..=20).fold(String::new(), written).into_bytes()
}

/// Vim for every want alike: no case here opens a region of another
/// shape.
fn table() -> Table {
    let vim = Editor::new();
    Box::new(move |want| match want {
        Want::Named("vim") | Want::Cell => {
            Some((String::from("vim"), vim.booted()))
        }
        _ => None,
    })
}

/// An editor over `README.md`, beside `other.md` for a second door and
/// a file whose own name holds a colon.
fn over(test: &str) -> (Rig, Tui, Rc<Host>) {
    let rig = Rig::new(test);
    rig.file("README.md", &twenty());
    // Line 2 is indented: a bare LINE lands on its first non-blank
    // and a LINE:COL on the column it names.
    rig.file("other.md", b"one\n  two\nthree\n");
    rig.file("a:b", b"colon\n");
    let host = rig.host();
    let tui = opened(&host, table(), Clip::none(), "", Some("README.md"), None)
        .unwrap();
    (rig, tui, host)
}

/// Where the cursor stands, in the grammar `point` spells: the one
/// reading of a landing this minor has, and the one the person gets.
fn at(tui: &mut Tui) -> String {
    typed(tui, ":point -c<CR>");
    said(tui)
}

#[test]
fn goto_lands_on_a_line_and_a_column() {
    let (_rig, mut tui, host) = over("locator-goto");
    typed(&mut tui, ":goto 12<CR>");
    assert_eq!(at(&mut tui), "/README.md:12:1");

    typed(&mut tui, ":goto 12:5<CR>");
    assert_eq!(at(&mut tui), "/README.md:12:5");

    // A range is a place too: it lands at its start.
    typed(&mut tui, ":goto 12-14<CR>");
    assert_eq!(at(&mut tui), "/README.md:12:1");
    typed(&mut tui, ":goto 12:5-14:3<CR>");
    assert_eq!(at(&mut tui), "/README.md:12:5");

    // Past the end is the last line, as `+N` is, and past a line's
    // end is its last char; the column landed on is the one wanted
    // from then on, as `begin` leaves it.
    typed(&mut tui, ":goto 99<CR>");
    assert_eq!(at(&mut tui), "/README.md:20:1");
    typed(&mut tui, ":goto 12:99<CR>");
    assert_eq!(at(&mut tui), "/README.md:12:7");
    typed(&mut tui, ":goto 12:5<CR>");
    typed(&mut tui, "j");
    assert_eq!(at(&mut tui), "/README.md:13:5");

    assert_eq!(listed(&host, "/self/vim/bin").len(), 25);
}

#[test]
fn an_unread_locator_is_said_and_moves_nothing() {
    let (_rig, mut tui, _host) = over("locator-unread");
    typed(&mut tui, "11G4l");
    typed(&mut tui, ":goto px12,34<CR>");
    assert_eq!(said(&mut tui), "E: invalid locator: px12,34");
    assert_eq!(at(&mut tui), "/README.md:11:5", "the cursor where it was");

    // Half a locator is no locator: the second end is read too.
    typed(&mut tui, ":goto 12-zz<CR>");
    assert_eq!(said(&mut tui), "E: invalid locator: 12-zz");
    assert_eq!(at(&mut tui), "/README.md:11:5");
}

#[test]
fn a_landing_is_a_jump() {
    let (_rig, mut tui, _host) = over("locator-jump");
    typed(&mut tui, "3G");
    typed(&mut tui, ":goto 12<CR>");
    assert_eq!(at(&mut tui), "/README.md:12:1");

    typed(&mut tui, "``");
    assert_eq!(at(&mut tui), "/README.md:3:1", "back where it left");
    typed(&mut tui, "``");
    assert_eq!(at(&mut tui), "/README.md:12:1");
}

#[test]
fn edit_splits_a_locator_once() {
    let (_rig, mut tui, _host) = over("locator-edit");
    typed(&mut tui, ":e other.md:2<CR>");
    assert_eq!(at(&mut tui), "/other.md:2:3", "the first non-blank");

    // A file whose own name holds the colon wins over the split.
    typed(&mut tui, ":e a:b<CR>");
    assert_eq!(at(&mut tui), "/a:b:1:1");

    // The file opens whatever the rest says; the rest is the view's
    // to refuse.
    typed(&mut tui, ":e README.md:px1<CR>");
    assert_eq!(said(&mut tui), "E: invalid locator: px1");
    assert_eq!(at(&mut tui), "/README.md:1:1");

    // The file already shown is not opened again; its locator lands
    // all the same.
    typed(&mut tui, ":e README.md:12<CR>");
    assert_eq!(at(&mut tui), "/README.md:12:1");

    // A name no file holds either whole or split is the new file it
    // asks for, colon and all, as `:e` always made one.
    typed(&mut tui, ":e new:file<CR>");
    assert_eq!(said(&mut tui), "\"new:file\" [New]");
}

/// `gF`, which no name here may spell: Rust wants a lower-case one.
#[test]
fn g_f_opens_what_is_under_the_cursor() {
    let (_rig, mut tui, _host) = over("locator-gf");
    typed(&mut tui, "ggOother.md:2:1<Esc>0");
    typed(&mut tui, "gF");
    assert_eq!(at(&mut tui), "/other.md:2:1", "the column, not the text");

    // Nothing under the cursor is nvim's own E446.
    typed(&mut tui, "Go<Esc>");
    typed(&mut tui, "gF");
    assert_eq!(said(&mut tui), "E446: No file name under cursor");
}

#[test]
fn the_ask_carries_the_locator() {
    let rig = Rig::new("locator-ask");
    rig.file("README.md", &twenty());
    let host = rig.host();
    // What the app leaves behind it: the path it split off, and the
    // rest, which it never read.
    let dev = dev::boot(
        &host,
        table(),
        Clip::none(),
        "",
        Some("README.md"),
        None,
        "12",
    )
    .unwrap();
    let mut tui = Tui::open(host, dev).unwrap();
    assert_eq!(frame(&mut tui).cursor.0, 11, "the first frame at line 12");
    assert_eq!(at(&mut tui), "/README.md:12:1");
}
