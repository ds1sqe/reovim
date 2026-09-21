//! Every key enters the engine as its `Press`.
//!
//! What the region read reaches the resolver whole — the clock and
//! the source the Client stamped — and every key no person typed
//! carries the press of the key that made it, so nothing of a
//! keyboard is invented and a recorded stream replays.

mod common;

use std::rc::Rc;

use common::one;
use reovim_client::Host;
use reovim_client::dev::Clip;
use reovim_client_api::{Key, keyboard, origin};
use reovim_client_tui::Tui;
use reovim_kernel_interface::{Calls, Handle, Rights};
use reovim_plugin_vim::Editor;
use reovim_rig::{Rig, lines, opened, said, typed};
use reovim_sdk::walk::ROOT;
use reovim_sdk::{cell, decode, ops};

/// An editor over `note`, whose text is `text`, with the Host its
/// keyboard is read through and the engine the presses reach.
fn over(name: &str, text: &[u8]) -> (Rig, Tui, Rc<Host>, Rc<Editor>) {
    let rig = Rig::new(name);
    rig.file("note", text);
    let host = rig.host();
    let editor = Editor::new();
    let tui = opened(
        &host,
        one(Rc::clone(&editor)),
        Clip::none(),
        "",
        Some("note"),
        None,
    )
    .unwrap();
    (rig, tui, host, editor)
}

/// A READ handle on the keyboard, opened at the end of the stream:
/// what is typed after it is what it reads, press by press, which is
/// the very press the Plugin's own handle reads.
fn watching(host: &Rc<Host>) -> Handle {
    let ctx = host.own();
    let rd = cell::resolve(&ctx, ROOT, "/self/dev/input/keyboard").unwrap();
    ctx.open(rd, Rights::READ).unwrap()
}

/// The next press that handle has not taken.
fn press(host: &Rc<Host>, h: Handle) -> keyboard::Press {
    let ctx = host.own();
    decode(&ops::read(&ctx, h, 0, 4096).unwrap()).unwrap()
}

/// One code straight onto the stream, for a key no notation spells.
fn wrote(host: &Rc<Host>, code: u64) {
    let ctx = host.own();
    let rd = cell::resolve(&ctx, ROOT, "/self/dev/input/keyboard").unwrap();
    let h = ctx.open(rd, Rights::WRITE).unwrap();
    ops::write(&ctx, h, 0, code.to_le_bytes().to_vec()).unwrap();
    ops::close(&ctx, h).unwrap();
}

/// The input the engine last took.
fn took(editor: &Rc<Editor>) -> Option<reovim_client_api::resolver::Input> {
    editor.engine().taken().cloned()
}

#[test]
fn a_typed_key_keeps_its_press() {
    let (_rig, mut tui, host, editor) = over("press-typed", b"abc\n");
    let watch = watching(&host);
    typed(&mut tui, "x");

    let stamped = press(&host, watch);
    let input = took(&editor).expect("the engine took the key");
    assert_eq!(input.origin, origin::TYPED);
    assert_eq!(input.press, stamped, "the whole press the Client stamped");
    assert_eq!(Key::of(input.press.key), Some(Key::Char('x')));
    assert_eq!(lines(&mut tui)[..1], ["bc"]);
}

#[test]
fn a_macros_keys_carry_the_press_of_the_at_key() {
    let (_rig, mut tui, host, editor) = over("press-macro", b"abcdef\n");
    typed(&mut tui, "qaxq");

    let watch = watching(&host);
    typed(&mut tui, "@a");
    let _at = press(&host, watch);
    let name = press(&host, watch);

    let input = took(&editor).expect("the fed key");
    assert_eq!(input.origin, origin::FED);
    assert_eq!(Key::of(input.press.key), Some(Key::Char('x')), "the run's key");
    assert_eq!(
        (input.press.when, input.press.source),
        (name.when, name.source),
        "the press of the key that made it"
    );
    assert_eq!(lines(&mut tui)[..1], ["cdef"]);
}

#[test]
fn a_repeats_keys_carry_the_press_of_the_dot() {
    let (_rig, mut tui, host, editor) = over("press-repeat", b"abcdef\n");
    typed(&mut tui, "x");

    let watch = watching(&host);
    typed(&mut tui, ".");
    let dot = press(&host, watch);

    let input = took(&editor).expect("the repeated key");
    assert_eq!(input.origin, origin::REPEAT);
    assert_eq!(Key::of(input.press.key), Some(Key::Char('x')));
    assert_eq!(
        (input.press.when, input.press.source),
        (dot.when, dot.source),
        "the press of the `.` that made it"
    );
    assert_eq!(lines(&mut tui)[..1], ["cdef"]);
}

#[test]
fn a_released_prefix_keeps_the_press_it_was_taken_with() {
    let (_rig, mut tui, host, editor) = over("press-released", b"one\ntwo\n");
    typed(&mut tui, ":bind dx write<CR>");

    let watch = watching(&host);
    typed(&mut tui, "d");
    let first = press(&host, watch);
    let held = took(&editor).expect("the held key");
    assert_eq!(held.origin, origin::TYPED);
    assert_eq!(held.press, first);

    typed(&mut tui, "d");
    let second = press(&host, watch);

    // The prefix let both keys go and they ran again. Each keeps the
    // press it was taken with, and whose it is: the origin the engine
    // reads for a released key is its maker's, a person's here.
    let input = took(&editor).expect("the key the prefix let go");
    assert_eq!(input.press, second, "the press it was taken with");
    assert_eq!(input.origin, origin::TYPED, "a person's key still");
    assert_eq!(lines(&mut tui)[..1], ["two"], "`dd` ran after all");
}

#[test]
fn an_unknown_code_is_nothing() {
    let (_rig, mut tui, host, editor) = over("press-unknown", b"abc\n");

    // No named key and no char: clients/core/api says `Key::of` has
    // none of it, and dropping it is the engine's to do.
    wrote(&host, 0x0011_000a);
    tui.tick().unwrap();
    assert_eq!(lines(&mut tui)[..1], ["abc"], "nothing of it");
    assert_eq!(said(&mut tui), "");
    assert!(took(&editor).is_none(), "no input was taken");

    typed(&mut tui, "x");
    assert_eq!(lines(&mut tui)[..1], ["bc"], "the queue moved on");
}

#[test]
fn the_budget_reads_the_origin() {
    let (_rig, mut tui, host, editor) = over("press-origin", b"abc\nh\n");
    typed(&mut tui, ":bind hz nohlsearch<CR>");

    // Typed at column 0: the `h` rings, and the `x` it let go is a
    // typed key still, so it takes its char.
    typed(&mut tui, "qahxq");
    assert_eq!(lines(&mut tui)[..1], ["bc"]);
    typed(&mut tui, "u");

    let watch = watching(&host);
    typed(&mut tui, "@a");
    let _at = press(&host, watch);
    let name = press(&host, watch);

    // Fed, the same two keys are the run's: what the prefix let go
    // carries the press the run was made with, and the bell drops it
    // with the rest of the run.
    let input = took(&editor).expect("the key the prefix let go");
    assert_eq!(input.origin, origin::FED, "let go, and still the run's");
    assert_eq!(
        (input.press.when, input.press.source),
        (name.when, name.source),
        "the press the run was made with"
    );
    assert_eq!(lines(&mut tui)[..1], ["abc"], "the run was dropped");

    // A key typed after a cancelled run is not dropped with it, and
    // the queue kept it as what it is.
    typed(&mut tui, "@ax");
    assert_eq!(lines(&mut tui)[..1], ["bc"]);
    assert_eq!(took(&editor).expect("the typed key").origin, origin::TYPED);
}
