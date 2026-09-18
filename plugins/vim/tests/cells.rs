//! Vim's own state under `/self/vim`: the mode, the keys of a command
//! that has not fired, and the registers, each made at the read out of
//! what the engine holds.
//!
//! The engine's cases (tests/vim.rs) read that state in Rust; these
//! follow it out through the tree, which is the way anything else at
//! this Client has to it. The last one reads a cell from inside Vim's
//! own frame, which is where the one `RefCell` would be held twice.

use std::rc::Rc;

use reovim_client::Host;
use reovim_client::dev::{self, Clip, Table, Want};
use reovim_client_tui::Tui;
use reovim_plugin_vim::notation::keys;
use reovim_rig::Rig;
use reovim_sdk::walk::ROOT;
use reovim_sdk::{cell, ops};

/// Vim for a cell; nothing else is opened here.
fn table() -> Table {
    Box::new(|want| match want {
        Want::Cell => Some((String::from("vim"), reovim_plugin_vim::boot())),
        _ => None,
    })
}

/// An editor over `note`, and the Host its cells are read through, as
/// another Plugin of this Client reads them.
fn over(test: &str) -> (Rig, Tui, Rc<Host>) {
    let rig = Rig::new(test);
    rig.file("note", b"one\ntwo\n");
    let host = rig.host();
    let dev = dev::boot(&host, table(), Clip::none(), "", Some("note"), None)
        .unwrap();
    let tui = Tui::open(host.clone(), dev).unwrap();
    (rig, tui, host)
}

/// Every key of `text` in Vim notation.
fn typed(tui: &mut Tui, text: &str) {
    for key in keys(text).unwrap() {
        tui.key(key).unwrap();
    }
}

/// What the cell `path` under `/self/vim` holds, as `cat` prints it.
fn cat(host: &Rc<Host>, path: &str) -> String {
    let ctx = host.own();
    let rd = cell::resolve(&ctx, ROOT, &format!("/self/vim/{path}")).unwrap();
    String::from_utf8(ops::bytes(&ctx, rd).unwrap()).unwrap()
}

/// The names `ls` lists under `path`, in the order the tree holds them.
fn ls(host: &Rc<Host>, path: &str) -> Vec<String> {
    let ctx = host.own();
    let rd = cell::resolve(&ctx, ROOT, path).unwrap();
    ops::children(&ctx, rd)
        .unwrap()
        .into_iter()
        .map(|child| ops::info(&ctx, child).unwrap().name.unwrap())
        .collect()
}

/// Vim's own word of the status row: what follows the name, the
/// position and the revision, which the tui writes before it.
fn said(tui: &mut Tui) -> String {
    let status = tui.frame(80, 24).unwrap().status;
    status.split_once("   ").unwrap().1.into()
}

#[test]
fn the_mode_cell_is_the_mode_the_keys_left() {
    let (_rig, mut tui, host) = over("cells-mode");
    let mode = || cat(&host, "mode");

    // A name and a newline, so `cat` prints one line; the status
    // row's own words are the statusline's business and stay there.
    assert_eq!(mode(), "normal\n");
    typed(&mut tui, "i");
    assert_eq!(mode(), "insert\n");
    assert_eq!(said(&mut tui), "-- INSERT --", "the row is not the cell");
    typed(&mut tui, "<Esc>v");
    assert_eq!(mode(), "visual\n");
    typed(&mut tui, "<Esc>V");
    assert_eq!(mode(), "visual line\n");
    typed(&mut tui, "<Esc><C-v>");
    assert_eq!(mode(), "visual block\n");
    typed(&mut tui, "<Esc>:");
    assert_eq!(mode(), "command\n");
    typed(&mut tui, "<Esc>");
    assert_eq!(mode(), "normal\n");
}

#[test]
fn the_pending_cell_holds_what_showcmd_would_show() {
    let (_rig, mut tui, host) = over("cells-pending");
    let pending = || cat(&host, "pending");

    assert_eq!(pending(), "", "nothing typed");
    typed(&mut tui, "d");
    assert_eq!(pending(), "d");
    typed(&mut tui, "i");
    assert_eq!(pending(), "di", "the key waiting for its second");
    typed(&mut tui, "w");
    assert_eq!(pending(), "", "the command fired");

    // The count before the operator is the operator's; the one after
    // it waits with the motion.
    typed(&mut tui, "2d3");
    assert_eq!(pending(), "2d3");
    typed(&mut tui, "w");
    assert_eq!(pending(), "");

    // What `.` repeats is the keys of an insert, which is a different
    // question and no part of this cell.
    typed(&mut tui, "ixy<Esc>");
    assert_eq!(pending(), "");

    // `"x` is the register the next operator uses, and it is shown.
    typed(&mut tui, "\"a");
    assert_eq!(pending(), "\"a");
}

#[test]
fn every_register_reads_what_vim_put_in_it() {
    let (_rig, mut tui, host) = over("cells-registers");
    let held = |name| cat(&host, &format!("registers/{name}"));

    // A linewise register ends in the newline it took, which is what
    // the clipboard is handed: the kind is in the bytes.
    typed(&mut tui, "\"ayy");
    assert_eq!(held('a'), "one\n");
    assert_eq!(held('"'), "one\n", "the unnamed one takes it too");
    typed(&mut tui, "\"byl");
    assert_eq!(held('b'), "o", "a charwise one ends where it ends");
    assert_eq!(held('a'), "one\n", "and `a` is where it was");
    assert_eq!(held('z'), "", "one nothing was put in reads empty");
}

#[test]
fn the_registers_are_listed_by_every_name_one_can_have() {
    let (_rig, _tui, host) = over("cells-listed");

    assert_eq!(
        ls(&host, "/self/vim"),
        ["view", "mode", "pending", "registers", "bin"]
    );
    let names = ls(&host, "/self/vim/registers");
    assert_eq!(names.len(), 27, "the unnamed one and `a` to `z`");
    assert_eq!(names[0], "\"");
    assert_eq!(names[26], "z");
    // `"+` is no register of Vim's: the clipboard is the Client's one
    // cell, and a second name for it would be a second clipboard.
    assert!(!names.iter().any(|name| name == "+"));
}

#[test]
fn a_cell_read_from_inside_vims_own_frame_finds_the_state_free() {
    let (rig, mut tui, host) = over("cells-reentry");
    // The Client's own coreutils, as the app loads it: the server's
    // `cat` would be handed a local rd, which is milestone 5's.
    let booted = reovim_module_coreutils::boot("");
    host.load("coreutils", booted.module, booted.drivers).unwrap();

    // The line runs inside the frame that took the key, and reads the
    // state that frame is running over: no borrow of it is held there,
    // so the read answers instead of panicking.
    typed(&mut tui, ":cat /self/vim/mode<CR>");
    assert_eq!(said(&mut tui), "normal", "the mode the line ran in");
    assert_eq!(cat(&host, "mode"), "normal\n", "and the cell is still it");

    // A name of Vim's own is a Driver under `/self/vim/bin`, so the
    // run comes back into this Plugin's `call` slot from inside the
    // same frame, and takes the state as that frame left it.
    typed(&mut tui, "ix<Esc>:w<CR>");
    assert_eq!(said(&mut tui), "", "the write said nothing");
    assert_eq!(rig.disk("note"), b"xone\ntwo\n");
}
