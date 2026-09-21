//! Vim's `"+` registers over `/self/dev/clipboard`, the whole way: a
//! yank reaches the machine the Client was given, a put takes what
//! another program left on it, and a Client given no machine says so.
//!
//! The engine's own case (tests/vim.rs) stops at what `clipped` holds;
//! this one follows the bytes out of the Plugin, through the cell, to
//! the store the rig stands in for the machine's tool.

use reovim_client::dev::{Clip, Table, Want};
use reovim_client_tui::Tui;
use reovim_rig::{Rig, lines, opened, said, typed};

/// Vim for a cell; nothing else is opened here. One `Editor` across
/// every swap, as the app's own table gives one (01.0).
fn table() -> Table {
    let vim = reovim_plugin_vim_host::Editor::new();
    Box::new(move |want| match want {
        Want::Cell => Some((String::from("vim"), vim.booted())),
        _ => None,
    })
}

/// An editor over `note`, the Client's machine being what `clip` makes
/// of the rig.
fn over(test: &str, clip: impl FnOnce(&Rig) -> Clip) -> (Rig, Tui) {
    let rig = Rig::new(test);
    rig.file("note", b"one\ntwo\n");
    let clip = clip(&rig);
    let tui =
        opened(&rig.host(), table(), clip, "", Some("note"), None).unwrap();
    (rig, tui)
}

#[test]
fn a_plus_yank_reaches_the_machine_and_a_plus_put_takes_what_it_holds() {
    let (rig, mut tui) = over("clip-round", Rig::clipping);

    // The line goes out whole, and nothing is said of it.
    typed(&mut tui, "\"+yy");
    assert_eq!(rig.clipboard(), b"one\n");
    assert_eq!(said(&mut tui), "");

    // What another program put there since is what the put puts.
    rig.clip(b"other\n");
    typed(&mut tui, "\"+p");
    assert_eq!(lines(&mut tui), ["one", "other", "two"]);
}

#[test]
fn a_client_given_no_machine_says_there_is_no_tool() {
    let (rig, mut tui) = over("clip-none", |_| Clip::none());

    // `Clip::none` answers what a host with no tool on PATH answers,
    // so the yank reaches Vim's own sentence by the one road.
    typed(&mut tui, "\"+yy");
    assert_eq!(said(&mut tui), "clipboard: no tool on PATH");
    assert_eq!(rig.clipboard(), b"", "nothing left the editor");

    // And a put has nothing to put, the text standing as it was.
    typed(&mut tui, "\"+p");
    assert_eq!(lines(&mut tui), ["one", "two"]);
}
