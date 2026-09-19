//! Vim over the draft `/self/dev/buffers` gave it: what a key costs
//! on the wire, and where a revision ends.
//!
//! Each case runs the Plugin through the tui on the rig, as a person
//! runs it, and counts what this Client sent with `Rig::sent()`,
//! against the one message a frame of its own spends.

use std::rc::Rc;

use reovim_client::dev::{Clip, Table};
use reovim_client::{Client, Host, Rd, SELF};
use reovim_client_api::Key;
use reovim_client_tui::{Step, Tui};
use reovim_plugin_vim::Editor;
use reovim_plugin_vim::notation::keys;
use reovim_rig::{Rig, lines, opened, said, status, typed};
use reovim_sdk::{cell, decode, encode, ops};
use reovim_text_api::{history, text};

/// The editor `file` opens in on `host`, its first frame drawn so that
/// no size is written inside a count below.
fn editor(host: &Rc<Host>, file: &str) -> Tui {
    let vim = Editor::new();
    let table: Table =
        Box::new(move |_| Some((String::from("vim"), vim.booted())));
    let mut tui =
        opened(host, table, Clip::none(), "", Some(file), None).unwrap();
    tui.frame(80, 24).unwrap();
    tui
}

/// What each key of `text` cost this Client on the wire.
fn cost(rig: &Rig, tui: &mut Tui, text: &str) -> Vec<u32> {
    let spent = |key| {
        let was = rig.sent();
        tui.key(key).unwrap();
        rig.sent() - was
    };
    keys(text).unwrap().into_iter().map(spent).collect()
}

/// What one frame with nothing typed costs, which is the tui's own
/// read of the event stream.
fn idle(rig: &Rig, tui: &mut Tui) -> u32 {
    let was = rig.sent();
    tui.tick().unwrap();
    rig.sent() - was
}

/// The Buffer of the server's the draft over `file` is over, which
/// modules/text names by the rd of the file itself.
fn buffer(client: &mut Client, file: &str) -> Rd {
    let source = client.resolve(&format!("/module/fs/root/{file}")).unwrap();
    client.resolve(&format!("/module/text/{}", source.0)).unwrap()
}

/// What that Buffer's history holds: the revisions a person would
/// undo.
fn revisions(client: &mut Client, buffer: Rd) -> history::Tree {
    let out = client.call_by_name(buffer, "tree", encode(&())).unwrap();
    decode(&out).unwrap()
}

/// What a stale draft says: nothing names the Buffer, the status row
/// already doing that.
const W12: &str = "W12: changed in another Client and here; :e! takes theirs";

/// `keys` as one write to the keyboard, so they land in the same
/// frame: what `tui.key` costs one write and one frame each, this
/// spends in a single one of either.
fn typed_together(host: &Rc<Host>, keys: &[Key]) {
    let ctx = host.own();
    let keyboard = cell::resolve(&ctx, SELF, "dev/input/keyboard").unwrap();
    let mut codes = Vec::new();
    for key in keys {
        codes.extend(key.code().to_le_bytes());
    }
    ops::put(&ctx, keyboard, codes).unwrap();
}

#[test]
fn a_key_in_insert_costs_no_message_and_the_esc_sends_them_all() {
    let rig = Rig::new("draft-insert");
    rig.file("note", b"one\n");
    let host = rig.host();
    let mut tui = editor(&host, "note");
    let mut other = rig.client();
    let buffer = buffer(&mut other, "note");

    let idle = idle(&rig, &mut tui);
    assert_eq!(idle, 1, "an idle frame is the tui's own read");
    let link = host.wire().link();
    assert_eq!(cost(&rig, &mut tui, "ihello"), [idle; 6], "no call at all");
    // What the counters a person reads say of the same six keys: the
    // tick's own `Since` a key and no byte beyond it. Then a frame
    // drawn for nobody, whose `display` write is the Client's own, is
    // not a message either, so no counter of the link moves at all.
    let keys = host.wire().link();
    assert_eq!(keys.messages - link.messages, u64::from(idle) * 6);
    let drawn = host.wire().link();
    assert_eq!(lines(&mut tui), ["helloone"], "and the screen has it");
    assert_eq!(host.wire().link(), drawn, "a frame nobody reads");
    assert_eq!(other.read_all(buffer).unwrap(), b"one\n", "nothing sent");
    assert!(status(&mut tui).contains("[+]"), "{}", status(&mut tui));

    // The Esc ends the revision: one message, holding every key.
    assert_eq!(cost(&rig, &mut tui, "<Esc>"), [idle + 1]);
    assert_eq!(other.read_all(buffer).unwrap(), b"helloone\n");
    let tree = revisions(&mut other, buffer);
    assert_eq!((tree.nodes.len(), tree.head), (2, 2), "one undo step");
}

#[test]
fn three_deletes_are_three_revisions_and_undo_walks_back_through_them() {
    let rig = Rig::new("draft-normal");
    rig.file("note", b"abcd\n");
    let mut tui = editor(&rig.host(), "note");
    let mut other = rig.client();
    let buffer = buffer(&mut other, "note");

    // An edit that joins nothing ends the revision before it, so each
    // `x` is a state of its own on the server.
    let idle = idle(&rig, &mut tui);
    assert_eq!(cost(&rig, &mut tui, "xxx"), [idle + 1; 3]);
    assert_eq!(revisions(&mut other, buffer).head, 4);

    // Undo and redo are the server's: the draft flushes, forwards and
    // reads the head back, four messages beside the frame's own.
    assert_eq!(cost(&rig, &mut tui, "uu"), [idle + 4; 2]);
    assert_eq!(lines(&mut tui), ["bcd"], "one revision at a time");
    assert_eq!(cost(&rig, &mut tui, "<C-r>"), [idle + 4]);
    assert_eq!(lines(&mut tui), ["cd"]);
    assert_eq!(status(&mut tui), "note  1:1  r3[+]   ");

    // A write flushes what is pending before it publishes the head;
    // what the first one costs past the save is the tui reading what
    // the write left.
    assert_eq!(typed(&mut tui, "ix<Esc>"), Step::Changed);
    assert_eq!(cost(&rig, &mut tui, ":w<CR>"), [idle, idle, idle + 3]);
    assert_eq!(rig.disk("note"), b"xcd\n");
    assert_eq!(status(&mut tui), "note  1:1  r5   ");
}

#[test]
fn the_second_flush_says_w12_and_keeps_the_typing() {
    let rig = Rig::new("draft-stale");
    rig.file("note", b"one\n");
    let mut first = editor(&rig.host(), "note");
    let mut second = editor(&rig.host(), "note");
    let mut other = rig.client();
    let buffer = buffer(&mut other, "note");

    // One pending in the first, uncommitted; the second's own flush,
    // at its Esc, lands first and moves the head.
    typed(&mut first, "A!");
    assert_eq!(typed(&mut second, "Ozero<Esc>"), Step::Changed);
    assert_eq!(other.read_all(buffer).unwrap(), b"zero\none\n");

    // The first one's own flush, at its Esc, finds the head moved:
    // refused, and said. What it typed stays on its own screen, and
    // nothing of it reaches the server.
    assert_eq!(typed(&mut first, "<Esc>"), Step::Changed);
    assert_eq!(said(&mut first), W12);
    assert_eq!(lines(&mut first), ["one!"]);
    assert_eq!(other.read_all(buffer).unwrap(), b"zero\none\n");

    // Stale from here on: `u` is a call straight on the open handle
    // and the flush it would have made costs no message, so it is an
    // idle frame's cost exactly. `:w`'s own dispatch, through the
    // shell, costs what it always does; the flush it would have made
    // costs no message either, one less than a save that lands (as
    // `three_deletes...` above spends `idle + 3` on a landing one).
    let idle = idle(&rig, &mut first);
    assert_eq!(cost(&rig, &mut first, "u"), [idle]);
    assert_eq!(said(&mut first), W12);
    assert_eq!(cost(&rig, &mut first, ":w<CR>"), [idle, idle, idle + 2]);
    assert_eq!(said(&mut first), W12);
    assert_eq!(rig.disk("note"), b"one\n", "neither Client has written it");

    // `:e!` takes the server's head and drops what was pending; the
    // yank taken before it survives, and a further edit and save land
    // again from there.
    typed(&mut first, "yy:e!<CR>p");
    assert_eq!(lines(&mut first), ["zero", "one!", "one"]);
    assert_eq!(typed(&mut first, ":w<CR>"), Step::Changed);
    assert_eq!(rig.disk("note"), b"zero\none!\none\n");
}

#[test]
fn an_edit_is_not_sent_after_a_flush_that_was_refused() {
    let rig = Rig::new("draft-two-keys");
    rig.file("note", b"one\n");
    let host = rig.host();
    let mut tui = editor(&host, "note");
    let mut other = rig.client();
    let buffer = buffer(&mut other, "note");

    // Another Client's run lands first, moving the head this Client's
    // next flush is taken against.
    let append = text::Edit { at: 4, len: 0, text: String::from("two\n") };
    other.call_by_name(buffer, "replace", encode(&append)).unwrap();

    // Two keys as one write, one tick: both land in the same frame,
    // so the flush the second `x` ends its own revision with — which
    // sends the first `x`'s edit — finds the head already moved, and
    // the second `x` is never sent at all.
    typed_together(&host, &[Key::Char('x'), Key::Char('x')]);
    assert_eq!(tui.tick().unwrap(), Step::Changed);

    assert_eq!(lines(&mut tui), ["ne"], "one x gone, not two");
    assert_eq!(said(&mut tui), W12);
    assert_eq!(
        other.read_all(buffer).unwrap(),
        b"one\ntwo\n",
        "the other's alone"
    );
}

#[test]
fn e_takes_what_another_client_did() {
    let rig = Rig::new("draft-e");
    rig.file("note", b"one\n");
    let mut tui = editor(&rig.host(), "note");
    let mut other = rig.client();
    let buffer = buffer(&mut other, "note");

    let append = text::Edit { at: 4, len: 0, text: String::from("two\n") };
    other.call_by_name(buffer, "replace", encode(&append)).unwrap();
    assert_eq!(lines(&mut tui), ["one"], "an idle draft does not follow it");

    assert_eq!(typed(&mut tui, ":e<CR>"), Step::Changed);
    assert_eq!(lines(&mut tui), ["one", "two"]);
    assert_eq!(said(&mut tui), "");
}
